//! 引擎：收抽象按鍵，回傳編輯區的狀態快照。不碰螢幕、檔案、網路與時間。
//!
//! 按鍵序列由前到後分成三個部分（見 ADR 0004）：
//!
//! 1. 釘住的段：使用者選定的字詞，顯示成漢字。
//! 2. 游標之前、還沒釘住的按鍵：只有這一部分交給 [`Lattice`] 猜測，候選字也只看它。
//! 3. 游標之後的按鍵：原封不動保留，等游標移過去才參與猜測。

use crate::lattice::{Config, Lattice, Stroke};
use crate::layout::Layout;
use crate::model::Model;
use crate::syllable::Slot;
use std::sync::Arc;

/// 選字狀態下一頁有幾個候選字，對應數字鍵 1～9、0。
pub const PAGE_SIZE: usize = 10;

/// 前端交給引擎的抽象按鍵。字元鍵一律用美式鍵盤「沒按 Shift 時」的字元表示，
/// 例如 `Char('h')`、`Char('4')`、`Char(' ')`、`Shift('1')`。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Char(char),
    Shift(char),
    Enter,
    ShiftEnter,
    Backspace,
    Escape,
    /// 切換中文模式與英文模式。
    ToggleMode,
    /// 游標往前、往後移一個按鍵。
    Left,
    Right,
    /// 游標往前、往後移一個音節。
    SyllableLeft,
    SyllableRight,
    /// 游標移到最後。
    End,
    /// 打字狀態下進入選字狀態並反白第二個候選字；選字狀態下把反白往下、往上移。
    Down,
    Up,
    /// 選字狀態下換到下一頁、上一頁候選字。
    PageDown,
    PageUp,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Chinese,
    English,
}

/// 一個候選字。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Candidate {
    pub text: String,
    /// `text` 裡第幾個字是容錯改過的（把某個按鍵當成它的相鄰鍵才得到的）。
    /// 只有整句猜測會標示。
    pub corrected: Vec<usize>,
}

/// 每次按鍵後引擎回傳的完整狀態。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Snapshot {
    /// 釘住的段，由前到後，每一段是使用者選定的文字。
    pub pinned: Vec<String>,
    /// 游標之前、還沒釘住的按鍵，照實際按的鍵顯示，一個音節一個字串、一個按鍵一個字元。
    /// 一聲（空白鍵）顯示成一個空格。
    pub before_cursor: Vec<String>,
    /// 游標之後的按鍵，格式同上。
    pub after_cursor: Vec<String>,
    /// 候選字。第一個是整句猜測，其餘是只涵蓋前面幾個音節的較短字詞。
    pub candidates: Vec<Candidate>,
    /// 選字狀態下反白的是第幾個候選字；`None` 代表打字狀態。
    pub highlighted: Option<usize>,
    /// 這次按鍵要送出給應用程式的文字；沒有就是空字串。
    pub committed: String,
    pub mode: Mode,
    /// 引擎有沒有處理這個按鍵。`false` 時前端應該讓按鍵照常交給應用程式。
    pub handled: bool,
    /// 這次操作釘住的字詞；之後要學習使用者習慣時從這裡接。
    pub newly_pinned: Option<String>,
}

impl Snapshot {
    /// 游標在最後時，現在按 Enter 會送出的文字：釘住的段加上整句猜測。
    pub fn text(&self) -> String {
        let mut text = self.pinned.concat();
        if let Some(first) = self.candidates.first() {
            text.push_str(&first.text);
        }
        text
    }

    /// 編輯區有沒有內容。
    pub fn is_empty(&self) -> bool {
        self.pinned.is_empty() && self.before_cursor.is_empty() && self.after_cursor.is_empty()
    }
}

/// 一個釘住的段：一組按鍵加上使用者選定的文字。
struct PinnedSegment {
    raw: Vec<char>,
    strokes: Vec<Stroke>,
    text: String,
    /// 這一段的最後一個詞，給後面的按鍵當前文；最後不是詞就沒有。
    context: Option<u32>,
}

/// 引擎內部的候選字：多記了它涵蓋到第幾個按鍵。
struct Choice {
    /// 涵蓋游標之前、還沒釘住的按鍵裡的 `0..end`。
    end: usize,
    text: String,
    corrected: Vec<usize>,
    context: Option<u32>,
}

/// 把按鍵依音節分組，回傳每一組有幾個按鍵。
struct Grouper {
    sizes: Vec<usize>,
    /// 最後一組還能不能再接按鍵，以及它是不是注音。
    open: Option<bool>,
    /// 聲調是否可以不打。可以的話，聲調鍵不再是音節唯一的結尾。
    toneless: bool,
    /// 最後一組已經有哪些位置（聲母、介音、韻母、聲調）的符號。
    filled: [bool; 4],
}

impl Grouper {
    fn new(toneless: bool) -> Grouper {
        Grouper { sizes: Vec::new(), open: None, toneless, filled: [false; 4] }
    }

    /// 引擎已經認定是一個音節的幾個按鍵。
    fn syllable(&mut self, keys: usize) {
        self.sizes.push(keys);
        self.open = None;
    }

    /// 還不屬於任何音節的一個按鍵，`slot` 是它的注音符號在音節裡的位置，原樣字元是 `None`。
    /// 連續的注音符號算一組，遇到聲調就結束；聲調可以不打時，同一個位置出現第二個符號
    /// （例如又一個聲母）也代表下一個音節開始了。連續的原樣字元算一組。
    fn loose(&mut self, slot: Option<Slot>) {
        let zhuyin = slot.is_some();
        let repeated = self.toneless && slot.is_some_and(|slot| self.filled[slot as usize]);
        match self.sizes.last_mut() {
            Some(size) if self.open == Some(zhuyin) && !repeated => *size += 1,
            _ => {
                self.sizes.push(1);
                self.filled = [false; 4];
            }
        }
        if let Some(slot) = slot {
            self.filled[slot as usize] = true;
        }
        self.open = (slot != Some(Slot::Tone)).then_some(zhuyin);
    }
}

pub struct Engine {
    /// 只涵蓋游標之前、還沒釘住的按鍵。
    lattice: Lattice,
    pinned: Vec<PinnedSegment>,
    /// `lattice` 裡每個按鍵原樣送出時的字元。
    before: Vec<char>,
    /// 游標之後的按鍵，倒著放：最後一個元素緊接在游標後面。
    after: Vec<(char, Stroke)>,
    /// 編輯區太長而自動送出的部分，最後一個詞是什麼。
    sent_context: Option<u32>,
    /// 選字狀態下反白的候選字。
    highlighted: Option<usize>,
    mode: Mode,
}

impl Engine {
    pub fn new(model: Arc<Model>, config: Config) -> Engine {
        Engine::with_layout(model, Layout::dachen(), config)
    }

    pub fn with_layout(model: Arc<Model>, layout: Layout, config: Config) -> Engine {
        Engine {
            lattice: Lattice::new(model, layout, config),
            pinned: Vec::new(),
            before: Vec::new(),
            after: Vec::new(),
            sent_context: None,
            highlighted: None,
            mode: Mode::Chinese,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn handle_key(&mut self, key: Key) -> Snapshot {
        let mut snapshot = match self.highlighted {
            Some(highlighted) => self.handle_selecting(key, highlighted),
            None => None,
        }
        .unwrap_or_else(|| {
            // 選字狀態不認得的按鍵：回到打字狀態，再照平常的方式處理。
            self.highlighted = None;
            self.handle_typing(key)
        });
        snapshot.committed.push_str(&self.commit_overflow());
        self.fill(snapshot)
    }

    /// 釘住第 `candidate` 個候選字。給滑鼠點選用，不需要先進入選字狀態。
    pub fn pin(&mut self, candidate: usize) -> Snapshot {
        let mut snapshot = self.blank();
        snapshot.newly_pinned = self.pin_choice(candidate);
        snapshot.handled = snapshot.newly_pinned.is_some();
        self.fill(snapshot)
    }

    pub fn snapshot(&self) -> Snapshot {
        self.fill(self.blank())
    }

    /// 選字狀態下處理按鍵。回傳 `None` 代表這個按鍵不是選字用的。
    fn handle_selecting(&mut self, key: Key, highlighted: usize) -> Option<Snapshot> {
        let count = self.choices().len();
        let last = count.checked_sub(1)?;
        let page_start = highlighted / PAGE_SIZE * PAGE_SIZE;
        let mut snapshot = self.blank();
        match key {
            Key::Down => self.highlighted = Some((highlighted + 1).min(last)),
            Key::Up => self.highlighted = Some(highlighted.saturating_sub(1)),
            Key::PageDown if page_start + PAGE_SIZE <= last => {
                self.highlighted = Some(page_start + PAGE_SIZE);
            }
            Key::PageUp if page_start >= PAGE_SIZE => {
                self.highlighted = Some(page_start - PAGE_SIZE);
            }
            Key::PageDown | Key::PageUp => {}
            Key::Escape => self.highlighted = None,
            Key::Char(digit) if digit.is_ascii_digit() => {
                // 數字鍵 1～9 是這一頁的第 1～9 個，0 是第 10 個。
                let offset = (digit as usize - '0' as usize + PAGE_SIZE - 1) % PAGE_SIZE;
                snapshot.newly_pinned = self.pin_choice(page_start + offset);
            }
            Key::Enter => self.adopt(highlighted, &mut snapshot),
            _ => return None,
        }
        if snapshot.newly_pinned.is_some() {
            // 選了一段之後留在選字狀態，接著選剩下的；沒有剩下的就回到打字狀態。
            self.highlighted = (!self.lattice.strokes().is_empty()).then_some(0);
        }
        Some(snapshot)
    }

    fn handle_typing(&mut self, key: Key) -> Snapshot {
        let empty = self.is_empty();
        let mut snapshot = self.blank();
        match key {
            Key::ToggleMode => {
                self.mode = match self.mode {
                    Mode::Chinese => Mode::English,
                    Mode::English => Mode::Chinese,
                };
            }
            Key::Char(ch) => match self.stroke_for_char(ch) {
                Some(stroke) => self.insert(ch, stroke),
                None => snapshot.handled = false,
            },
            Key::Shift(ch) => {
                let shifted = shifted_ascii(ch);
                match self.mode {
                    Mode::English if empty => snapshot.handled = false,
                    Mode::English => self.insert(shifted, Stroke::Literal(shifted)),
                    Mode::Chinese => {
                        let text = full_width_punctuation(ch).unwrap_or(shifted);
                        self.insert(shifted, Stroke::Literal(text));
                    }
                }
            }
            // 編輯區是空的時候，編輯用的按鍵都交還給應用程式。
            _ if empty => snapshot.handled = false,
            Key::Enter => self.adopt(0, &mut snapshot),
            Key::ShiftEnter => {
                let pinned = self.pinned.iter().flat_map(|segment| segment.raw.iter());
                let after = self.after.iter().rev().map(|(raw, _)| raw);
                snapshot.committed = pinned.chain(&self.before).chain(after).collect();
                self.clear();
            }
            Key::Backspace => {
                // 游標在釘住的段的邊界時，先撤銷最後一次釘住，不刪按鍵。
                if self.before.pop().is_some() {
                    self.lattice.pop();
                } else {
                    self.unpin();
                }
            }
            Key::Escape => self.clear(),
            Key::Left => {
                self.unpin_if_at_boundary();
                self.move_left(1);
            }
            Key::Right => self.move_right(1),
            Key::SyllableLeft => {
                self.unpin_if_at_boundary();
                let keys = self.groups_before().last().copied().unwrap_or(0);
                self.move_left(keys);
            }
            Key::SyllableRight => {
                let keys = self.groups_after().first().copied().unwrap_or(0);
                self.move_right(keys);
            }
            Key::End => self.move_right(self.after.len()),
            // 進入選字狀態時直接反白第二個：要第一個的話直接按 Enter 就好，不需要進來選。
            Key::Down => {
                self.highlighted = self.choices().len().checked_sub(1).map(|last| last.min(1));
            }
            Key::Up | Key::PageDown | Key::PageUp => {}
        }
        snapshot
    }

    fn is_empty(&self) -> bool {
        self.pinned.is_empty() && self.before.is_empty() && self.after.is_empty()
    }

    /// 還沒填入編輯區內容的快照。
    fn blank(&self) -> Snapshot {
        Snapshot {
            pinned: Vec::new(),
            before_cursor: Vec::new(),
            after_cursor: Vec::new(),
            candidates: Vec::new(),
            highlighted: None,
            committed: String::new(),
            mode: self.mode,
            handled: true,
            newly_pinned: None,
        }
    }

    /// 把目前的編輯區內容填進快照。
    fn fill(&self, mut snapshot: Snapshot) -> Snapshot {
        let strokes = self.lattice.strokes();
        let after: Vec<Stroke> = self.after.iter().rev().map(|&(_, stroke)| stroke).collect();
        snapshot.pinned = self.pinned.iter().map(|segment| segment.text.clone()).collect();
        snapshot.before_cursor = self.shown_groups(strokes, &self.groups_before());
        snapshot.after_cursor = self.shown_groups(&after, &self.groups_after());
        snapshot.candidates = self
            .choices()
            .into_iter()
            .map(|choice| Candidate { text: choice.text, corrected: choice.corrected })
            .collect();
        snapshot.highlighted = self.highlighted;
        snapshot.mode = self.mode;
        snapshot
    }

    /// 決定一個字元鍵在目前模式下要變成按鍵序列裡的什麼；`None` 代表引擎不處理。
    fn stroke_for_char(&self, ch: char) -> Option<Stroke> {
        let empty = self.is_empty();
        if self.mode == Mode::English {
            return (!empty).then_some(Stroke::Literal(ch));
        }
        let layout = self.lattice.layout();
        if ch == ' ' {
            // 空白鍵接在還沒打完的音節後面是一聲，否則就是一個空格。
            let pending = match self.lattice.strokes().last() {
                Some(Stroke::Zhuyin(key)) => layout.symbol(*key).is_some_and(|s| s.slot != Slot::Tone),
                _ => false,
            };
            return match (empty, pending) {
                (true, _) => None,
                (false, true) => Some(Stroke::Zhuyin(' ')),
                (false, false) => Some(Stroke::Literal(' ')),
            };
        }
        if layout.symbol(ch).is_some() {
            return Some(Stroke::Zhuyin(ch));
        }
        match unshifted_punctuation(ch) {
            Some(mark) => Some(Stroke::Literal(mark)),
            None => (!empty).then_some(Stroke::Literal(ch)),
        }
    }

    /// 在游標的位置加入一個按鍵。
    fn insert(&mut self, raw: char, stroke: Stroke) {
        self.before.push(raw);
        self.lattice.push(stroke);
    }

    fn move_left(&mut self, keys: usize) {
        for _ in 0..keys {
            let (Some(raw), Some(&stroke)) = (self.before.pop(), self.lattice.strokes().last())
            else {
                break;
            };
            self.lattice.pop();
            self.after.push((raw, stroke));
        }
    }

    fn move_right(&mut self, keys: usize) {
        for _ in 0..keys {
            let Some((raw, stroke)) = self.after.pop() else { break };
            self.insert(raw, stroke);
        }
    }

    fn clear(&mut self) {
        self.pinned.clear();
        self.before.clear();
        self.after.clear();
        self.sent_context = None;
        self.highlighted = None;
        self.lattice.clear();
    }

    /// 釘住的段之後的按鍵，猜測時要參考的前一個詞。
    fn context(&self) -> Option<u32> {
        match self.pinned.last() {
            Some(segment) => segment.context,
            None => self.sent_context,
        }
    }

    /// 釘住第 `index` 個候選字，回傳釘住的文字；沒有這個候選字就什麼都不做。
    /// 游標之前的按鍵都釘完時，游標跳到最後，讓之後的按鍵接著猜（同 RIME）。
    fn pin_choice(&mut self, index: usize) -> Option<String> {
        let choice = self.choices().into_iter().nth(index)?;
        self.pinned.push(PinnedSegment {
            raw: self.before.drain(..choice.end).collect(),
            strokes: self.lattice.strokes()[..choice.end].to_vec(),
            text: choice.text.clone(),
            context: choice.context,
        });
        self.lattice.drop_prefix(choice.end, choice.context);
        if self.before.is_empty() {
            self.move_right(self.after.len());
        }
        Some(choice.text)
    }

    /// Enter：採用第 `index` 個候選字；採用後沒有剩下的按鍵就送出（見 ADR 0006）。
    fn adopt(&mut self, index: usize, snapshot: &mut Snapshot) {
        if self.after.is_empty() && index == 0 {
            // 採用整句猜測後一定什麼都不剩，直接送出。不經過釘住，因為這不是使用者特地選的字詞。
            snapshot.committed = self.send();
        } else if self.before.is_empty() {
            // 游標之前沒有可以採用的按鍵：只把游標移到最後。
            self.move_right(self.after.len());
        } else {
            snapshot.newly_pinned = self.pin_choice(index);
            if self.before.is_empty() && self.after.is_empty() {
                snapshot.committed = self.send();
            }
        }
    }

    /// 撤銷最後一次釘住：那一段的按鍵退回成還沒釘住的按鍵。
    fn unpin(&mut self) {
        let Some(mut segment) = self.pinned.pop() else { return };
        segment.strokes.extend_from_slice(self.lattice.strokes());
        segment.raw.append(&mut self.before);
        self.before = segment.raw;
        self.lattice.rebuild(segment.strokes, self.context());
    }

    /// 游標要往前移、而它前面緊接著釘住的段時，先撤銷那一段的釘住。
    fn unpin_if_at_boundary(&mut self) {
        if self.before.is_empty() {
            self.unpin();
        }
    }

    /// 送出：釘住的段，加上其餘所有按鍵（包含游標之後的）的整句猜測。
    fn send(&mut self) -> String {
        self.move_right(self.after.len());
        let mut text: String = self.pinned.iter().map(|segment| segment.text.as_str()).collect();
        if let Some(whole) = self.whole() {
            text.push_str(&whole.text);
        }
        self.clear();
        text
    }

    /// 一個還沒釘住的按鍵在編輯區裡顯示成什麼字元。
    fn shown(&self, stroke: Stroke) -> char {
        match stroke {
            Stroke::Literal(ch) => ch,
            Stroke::Zhuyin(key) => {
                self.lattice.layout().symbol(key).and_then(|s| s.to_char()).unwrap_or(' ')
            }
        }
    }

    fn shown_groups(&self, strokes: &[Stroke], sizes: &[usize]) -> Vec<String> {
        let mut rest = strokes;
        sizes
            .iter()
            .map(|&size| {
                let (group, tail) = rest.split_at(size);
                rest = tail;
                group.iter().map(|&stroke| self.shown(stroke)).collect()
            })
            .collect()
    }

    fn grouper(&self) -> Grouper {
        Grouper::new(self.lattice.config().toneless_penalty.is_some())
    }

    fn add_loose(&self, grouper: &mut Grouper, stroke: Stroke) {
        match stroke {
            Stroke::Literal(_) => grouper.loose(None),
            Stroke::Zhuyin(key) => {
                grouper.loose(self.lattice.layout().symbol(key).map(|symbol| symbol.slot))
            }
        }
    }

    /// 游標之前、還沒釘住的按鍵依音節分組：引擎猜成詞的部分照它的切法，
    /// 其餘照 [`Grouper::loose`] 的規則。
    fn groups_before(&self) -> Vec<usize> {
        let mut grouper = self.grouper();
        for segment in self.lattice.best_path() {
            if segment.word.is_none() {
                self.add_loose(&mut grouper, self.lattice.strokes()[segment.start]);
                continue;
            }
            let ends = segment.syllable_starts.iter().skip(1).chain([&segment.end]);
            for (start, end) in segment.syllable_starts.iter().zip(ends) {
                grouper.syllable(end - start);
            }
        }
        grouper.sizes
    }

    /// 游標之後的按鍵沒有猜測可以依據，一律照 [`Grouper::loose`] 的規則分組。
    fn groups_after(&self) -> Vec<usize> {
        let mut grouper = self.grouper();
        for &(_, stroke) in self.after.iter().rev() {
            self.add_loose(&mut grouper, stroke);
        }
        grouper.sizes
    }

    /// 整句猜測：游標之前、還沒釘住的所有按鍵一起考慮後最可能的句子。
    fn whole(&self) -> Option<Choice> {
        self.whole_and_leading().map(|(whole, _)| whole)
    }

    /// 整句猜測，以及它開頭「不是詞」的那一段（標點、猜不出來的注音）有幾個按鍵。
    fn whole_and_leading(&self) -> Option<(Choice, usize)> {
        let strokes = self.lattice.strokes();
        if strokes.is_empty() {
            return None;
        }
        let model = self.lattice.model();
        let mut whole =
            Choice { end: strokes.len(), text: String::new(), corrected: Vec::new(), context: None };
        let mut length = 0;
        let path = self.lattice.best_path();
        let leading = path.iter().take_while(|s| s.word.is_none()).last().map_or(0, |s| s.end);
        for segment in path {
            match segment.word {
                Some(word) => {
                    for (index, ch) in model.word(word).chars().enumerate() {
                        if segment.syllable_typos.get(index).is_some_and(|&typos| typos > 0) {
                            whole.corrected.push(length);
                        }
                        whole.text.push(ch);
                        length += 1;
                    }
                }
                None => {
                    whole.text.push(self.shown(strokes[segment.start]));
                    length += 1;
                }
            }
            whole.context = segment.word;
        }
        Some((whole, leading))
    }

    /// 所有候選字：整句猜測在最前面，後面是從第一個按鍵開始、比較短的字詞。
    fn choices(&self) -> Vec<Choice> {
        let Some((whole, leading)) = self.whole_and_leading() else { return Vec::new() };
        let strokes = self.lattice.strokes();
        let model = self.lattice.model();
        let mut choices = Vec::new();
        // 開頭是標點或猜不出來的注音時，讓使用者可以先把它單獨釘住，再選後面的字。
        if leading > 0 && leading < whole.end {
            choices.push(Choice {
                end: leading,
                text: strokes[..leading].iter().map(|&stroke| self.shown(stroke)).collect(),
                corrected: Vec::new(),
                context: None,
            });
        }
        for candidate in self.lattice.candidates(0) {
            let text = model.word(candidate.word);
            if candidate.end == whole.end && text == whole.text {
                continue;
            }
            choices.push(Choice {
                end: candidate.end,
                text: text.to_string(),
                corrected: Vec::new(),
                context: Some(candidate.word),
            });
        }
        choices.insert(0, whole);
        choices
    }

    /// 編輯區超過長度上限時，把最前面的幾段送出，回傳送出的文字。
    fn commit_overflow(&mut self) -> String {
        let mut committed = String::new();
        let max = self.lattice.config().max_cells;
        loop {
            let model = self.lattice.model();
            let path = self.lattice.best_path();
            let pinned: usize = self.pinned.iter().map(|s| s.text.chars().count()).sum();
            let guessed: usize =
                path.iter().map(|s| s.word.map_or(1, |word| model.word(word).chars().count())).sum();
            if pinned + guessed <= max {
                break;
            }
            if !self.pinned.is_empty() {
                let segment = self.pinned.remove(0);
                committed.push_str(&segment.text);
                if self.pinned.is_empty() {
                    self.sent_context = segment.context;
                }
                continue;
            }
            let Some(first) = path.first() else { break };
            match first.word {
                Some(word) => committed.push_str(model.word(word)),
                None => committed.push(self.shown(self.lattice.strokes()[first.start])),
            }
            self.before.drain(..first.end);
            self.sent_context = first.word;
            self.lattice.drop_prefix(first.end, first.word);
        }
        committed
    }
}

/// 美式鍵盤上按住 Shift 時打出的字元。
fn shifted_ascii(ch: char) -> char {
    const PLAIN: &str = "`1234567890-=[]\\;',./";
    const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";
    match PLAIN.chars().position(|c| c == ch) {
        Some(i) => SHIFTED.chars().nth(i).unwrap(),
        None => ch.to_ascii_uppercase(),
    }
}

/// 中文模式下 Shift 加某個鍵對應的全形標點。
fn full_width_punctuation(ch: char) -> Option<char> {
    const KEYS: &str = ",./;1'90[]\\-";
    const MARKS: &str = "，。？：！；（）「」、—";
    KEYS.chars().position(|c| c == ch).and_then(|i| MARKS.chars().nth(i))
}

/// 中文模式下，沒被注音占用的鍵直接對應的全形標點。
fn unshifted_punctuation(ch: char) -> Option<char> {
    match ch {
        '\'' | '\\' => Some('、'),
        '[' => Some('「'),
        ']' => Some('」'),
        _ => None,
    }
}
