//! 引擎：收抽象按鍵，回傳編輯區的狀態快照。不碰螢幕、檔案、網路與時間。

use crate::lattice::{Config, Lattice, Pin, SegmentKind, Stroke};
use crate::layout::Layout;
use crate::model::Model;
use crate::syllable::Slot;
use std::sync::Arc;

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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Chinese,
    English,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellKind {
    /// 引擎猜出來的字。
    Guess,
    /// 使用者釘住的字。
    Pinned,
    /// 猜不出來或還沒打完，照實顯示的注音符號。
    Raw,
    /// 原樣顯示的字元（英文字母、標點等）。
    Literal,
}

/// 編輯區裡的一格：一個漢字、一個注音符號，或一個原樣字元。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cell {
    pub text: String,
    pub kind: CellKind,
}

/// 每次按鍵後引擎回傳的完整狀態。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Snapshot {
    /// 編輯區目前的內容。
    pub cells: Vec<Cell>,
    /// 這次按鍵要送出給應用程式的文字；沒有就是空字串。
    pub committed: String,
    pub mode: Mode,
    /// 引擎有沒有處理這個按鍵。`false` 時前端應該讓按鍵照常交給應用程式。
    pub handled: bool,
    /// 這次操作釘住的字詞；之後要學習使用者習慣時從這裡接。
    pub pinned: Option<String>,
}

impl Snapshot {
    /// 編輯區的文字。
    pub fn text(&self) -> String {
        self.cells.iter().map(|c| c.text.as_str()).collect()
    }
}

/// 一格對應到按鍵序列的哪一段。
struct CellSpan {
    cell: Cell,
    start: usize,
}

pub struct Engine {
    lattice: Lattice,
    /// 按鍵序列裡每個按鍵原樣送出時的字元。
    raw: Vec<char>,
    mode: Mode,
}

impl Engine {
    pub fn new(model: Arc<Model>, config: Config) -> Engine {
        Engine::with_layout(model, Layout::dachen(), config)
    }

    pub fn with_layout(model: Arc<Model>, layout: Layout, config: Config) -> Engine {
        Engine { lattice: Lattice::new(model, layout, config), raw: Vec::new(), mode: Mode::Chinese }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn handle_key(&mut self, key: Key) -> Snapshot {
        let empty = self.raw.is_empty();
        let mut committed = String::new();
        let mut handled = true;
        match key {
            Key::ToggleMode => {
                self.mode = match self.mode {
                    Mode::Chinese => Mode::English,
                    Mode::English => Mode::Chinese,
                };
            }
            Key::Enter | Key::ShiftEnter | Key::Backspace | Key::Escape if empty => handled = false,
            Key::Enter => {
                committed = self.text();
                self.clear();
            }
            Key::ShiftEnter => {
                committed = self.raw.iter().collect();
                self.clear();
            }
            Key::Backspace => {
                self.raw.pop();
                self.lattice.pop();
            }
            Key::Escape => self.clear(),
            Key::Char(ch) => match self.stroke_for_char(ch) {
                Some(stroke) => self.push(ch, stroke),
                None => handled = false,
            },
            Key::Shift(ch) => {
                let shifted = shifted_ascii(ch);
                match self.mode {
                    Mode::English if empty => handled = false,
                    Mode::English => self.push(shifted, Stroke::Literal(shifted)),
                    Mode::Chinese => {
                        let text = full_width_punctuation(ch).unwrap_or(shifted);
                        self.push(shifted, Stroke::Literal(text));
                    }
                }
            }
        }
        committed.push_str(&self.commit_overflow());
        Snapshot { cells: self.cells(), committed, mode: self.mode, handled, pinned: None }
    }

    /// 編輯區第 `cell` 格可以選的候選字詞。
    pub fn candidates(&self, cell: usize) -> Vec<String> {
        let model = self.lattice.model();
        self.candidates_at(cell).iter().map(|c| model.word(c.word).to_string()).collect()
    }

    /// 把第 `cell` 格釘住成第 `candidate` 個候選字詞。
    pub fn pin(&mut self, cell: usize, candidate: usize) -> Snapshot {
        let mut pinned = None;
        if let Some(chosen) = self.candidates_at(cell).get(candidate).copied() {
            pinned = Some(self.lattice.model().word(chosen.word).to_string());
            self.lattice.pin(Pin { start: chosen.start, end: chosen.end, word: chosen.word });
        }
        Snapshot {
            cells: self.cells(),
            committed: String::new(),
            mode: self.mode,
            handled: pinned.is_some(),
            pinned,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            cells: self.cells(),
            committed: String::new(),
            mode: self.mode,
            handled: true,
            pinned: None,
        }
    }

    fn candidates_at(&self, cell: usize) -> Vec<crate::lattice::Candidate> {
        match self.cell_spans().get(cell) {
            Some(span) if span.cell.kind == CellKind::Guess || span.cell.kind == CellKind::Pinned => {
                self.lattice.candidates(span.start)
            }
            _ => Vec::new(),
        }
    }

    /// 決定一個字元鍵在目前模式下要變成按鍵序列裡的什麼；`None` 代表引擎不處理。
    fn stroke_for_char(&self, ch: char) -> Option<Stroke> {
        let empty = self.raw.is_empty();
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

    fn push(&mut self, raw: char, stroke: Stroke) {
        self.raw.push(raw);
        self.lattice.push(stroke);
    }

    fn clear(&mut self) {
        self.raw.clear();
        self.lattice.clear();
    }

    fn text(&self) -> String {
        self.cells().into_iter().map(|c| c.text).collect()
    }

    fn cells(&self) -> Vec<Cell> {
        self.cell_spans().into_iter().map(|s| s.cell).collect()
    }

    fn cell_spans(&self) -> Vec<CellSpan> {
        let model = self.lattice.model();
        let layout = self.lattice.layout();
        let mut spans = Vec::new();
        for segment in self.lattice.best_path() {
            match (segment.kind, self.lattice.strokes()[segment.start]) {
                (SegmentKind::Word, _) => {
                    let kind = if segment.pinned { CellKind::Pinned } else { CellKind::Guess };
                    let word = model.word(segment.word.expect("詞的那一段一定有詞"));
                    for (ch, &start) in word.chars().zip(&segment.syllable_starts) {
                        spans.push(CellSpan { cell: Cell { text: ch.to_string(), kind }, start });
                    }
                }
                (_, Stroke::Literal(ch)) => spans.push(CellSpan {
                    cell: Cell { text: ch.to_string(), kind: CellKind::Literal },
                    start: segment.start,
                }),
                (_, Stroke::Zhuyin(key)) => {
                    let shown = layout.symbol(key).and_then(|s| s.to_char()).unwrap_or(' ');
                    spans.push(CellSpan {
                        cell: Cell { text: shown.to_string(), kind: CellKind::Raw },
                        start: segment.start,
                    });
                }
            }
        }
        spans
    }

    /// 編輯區超過長度上限時，把最前面的幾段送出，回傳送出的文字。
    fn commit_overflow(&mut self) -> String {
        let mut committed = String::new();
        let max = self.lattice.config().max_cells;
        while self.cell_spans().len() > max {
            let Some(first) = self.lattice.best_path().into_iter().next() else { break };
            let cut = first.end;
            let cells = self.cell_spans();
            committed.extend(cells.iter().take_while(|s| s.start < cut).map(|s| s.cell.text.as_str()));
            self.raw.drain(..cut);
            let strokes = self.lattice.strokes()[cut..].to_vec();
            let pins = self
                .lattice
                .pins()
                .iter()
                .filter(|p| p.start >= cut)
                .map(|p| Pin { start: p.start - cut, end: p.end - cut, word: p.word })
                .collect();
            self.lattice.rebuild(strokes, pins);
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
