//! 猜句子的核心：把按鍵序列所有可能的解讀排成一張圖，找出總分最高的路徑。
//!
//! 圖上的「位置」是按鍵之間的空隙：位置 0 在第一個按鍵之前，位置 n 在第 n 個按鍵之後。
//! 每多一個按鍵，只需要算出「結束在新位置」的解讀，之前的結果都不會變，
//! 所以每次按鍵的運算量和編輯區的長度無關。

use crate::layout::Layout;
use crate::model::Model;
use crate::syllable::{Slot, Syllable};
use std::collections::HashSet;
use std::sync::Arc;

const NONE: u32 = u32::MAX;
/// 一個音節最少與最多由幾個按鍵組成（含聲調鍵）。
const MIN_SYLLABLE_KEYS: usize = 2;
const MAX_SYLLABLE_KEYS: usize = 4;
/// 不打聲調時，一個音節最少與最多由幾個按鍵組成。
const MIN_TONELESS_KEYS: usize = 1;
const MAX_TONELESS_KEYS: usize = 3;
/// 候選字詞裡，誤觸讀法最多列出幾個。
const MAX_TYPO_CANDIDATES: usize = 30;

/// 影響猜測結果的參數。分數都是 log10 機率，越接近 0 越好。
#[derive(Clone, Debug)]
pub struct Config {
    /// 每一次誤觸的扣分。這個數值是用準確率測試調出來的：扣太少會把打對的字改錯，
    /// 扣太多則救不回誤觸。
    pub typo_penalty: f32,
    /// 一個音節裡最多容許幾次誤觸；0 代表關閉容錯。
    pub max_typos_per_syllable: u8,
    /// 一個詞裡最多容許幾次誤觸。
    pub max_typos_per_word: u8,
    /// 把一個按鍵當成「猜不出來，照實顯示注音符號」的扣分，要比任何合理的解讀都差。
    pub raw_penalty: f32,
    /// 同一段讀音最多考慮幾個詞（沒有誤觸的解讀）。
    pub words_per_span: usize,
    /// 同一段讀音最多考慮幾個詞（含誤觸的解讀）。
    pub words_per_typo_span: usize,
    /// 每個位置最多保留幾條路徑。
    pub beam: usize,
    /// 編輯區最多幾個字，超過時最前面的部分會自動送出。
    pub max_cells: usize,
    /// 每個沒打聲調的音節扣幾分；`None` 代表聲調一定要打（見 ADR 0005）。
    /// 沒有聲調鍵的音節會被當成「任何一個聲調都有可能」，扣一點分讓有打聲調的讀法優先。
    /// 這個數值是用準確率測試調出來的：扣太多會猜不出沒打聲調的句子，
    /// 扣太少則會把「打了聲調但有誤觸」的句子誤認成沒打聲調。
    pub toneless_penalty: Option<f32>,
    /// 沒打聲調的音節裡最多容許幾次誤觸。容許 2 次的準確率和 1 次幾乎一樣，但慢很多。
    /// 只有一個符號的音節一律不容錯。
    pub max_typos_per_toneless_syllable: u8,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            typo_penalty: -3.5,
            max_typos_per_syllable: 2,
            max_typos_per_word: 3,
            raw_penalty: -20.0,
            words_per_span: 40,
            words_per_typo_span: 10,
            beam: 80,
            max_cells: 50,
            toneless_penalty: Some(-0.5),
            max_typos_per_toneless_syllable: 1,
        }
    }
}

/// 按鍵序列裡的一個按鍵，在引擎眼中的兩種身分。
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stroke {
    /// 中文模式下按的注音鍵，要參與猜測。
    Zhuyin(char),
    /// 不參與猜測、原樣顯示的字元（英文字母、標點等）。
    Literal(char),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SegmentKind {
    /// 猜出來的詞。
    Word,
    /// 猜不出來、照實顯示的單一注音鍵。
    Raw,
    /// 原樣顯示的字元。
    Literal,
}

/// 最佳路徑上的一段。
#[derive(Clone, Debug)]
pub struct Segment {
    pub start: usize,
    pub end: usize,
    pub kind: SegmentKind,
    pub word: Option<u32>,
    /// 這一段裡每個音節從哪個位置開始（只有 `Word` 有意義）。
    pub syllable_starts: Vec<usize>,
    /// 每個音節用了幾次誤觸的解讀，和 `syllable_starts` 一一對應。
    pub syllable_typos: Vec<u8>,
}

/// 某個位置可以選的一個候選字詞。
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub start: usize,
    pub end: usize,
    pub word: u32,
    pub typos: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NodeKind {
    Start,
    Word,
    Raw,
    Literal,
}

/// 一條「結束在某個位置」的路徑的最後一段。
#[derive(Clone, Copy, PartialEq, Debug)]
struct Node {
    start: u32,
    /// 前一段是 `nodes[start]` 裡的第幾個。
    back: u32,
    kind: NodeKind,
    word: u32,
    /// 對應 `partials[結束位置]` 裡的第幾個，用來找回音節的邊界。
    partial: u32,
    score: f32,
}

/// 讀音樹上走到一半的進度：從 `start` 開始的按鍵，目前讀到 `node`。
#[derive(Clone, Copy, PartialEq, Debug)]
struct Partial {
    start: u32,
    /// 最後一個音節從哪裡開始。
    syllable_start: u32,
    /// 前一個音節的進度是 `partials[syllable_start]` 裡的第幾個；第一個音節是 `NONE`。
    parent: u32,
    node: u32,
    typos: u8,
    /// 從詞的開頭累計，有幾個音節沒打聲調。
    toneless: u8,
    /// 最後一個音節沒打聲調，所以它在哪裡結束並不確定。
    open: bool,
}

pub struct Lattice {
    model: Arc<Model>,
    layout: Layout,
    config: Config,
    strokes: Vec<Stroke>,
    partials: Vec<Vec<Partial>>,
    nodes: Vec<Vec<Node>>,
    /// 按鍵序列之前的最後一個詞，讓第一個詞也能參考前文；沒有就是 `NONE`。
    context: u32,
}

impl Lattice {
    pub fn new(model: Arc<Model>, layout: Layout, config: Config) -> Lattice {
        let mut lattice = Lattice {
            model,
            layout,
            config,
            strokes: Vec::new(),
            partials: Vec::new(),
            nodes: Vec::new(),
            context: NONE,
        };
        lattice.reset();
        lattice
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn strokes(&self) -> &[Stroke] {
        &self.strokes
    }

    /// 路徑的起點。它帶著前文的詞，之後算分數時第一個詞就會自動參考它。
    fn start_node(&self) -> Node {
        Node {
            start: 0,
            back: NONE,
            kind: NodeKind::Start,
            word: self.context,
            partial: NONE,
            score: 0.0,
        }
    }

    fn reset(&mut self) {
        self.strokes.clear();
        self.partials = vec![Vec::new()];
        self.nodes = vec![vec![self.start_node()]];
    }

    /// 清空按鍵序列與前文。
    pub fn clear(&mut self) {
        self.context = NONE;
        self.reset();
    }

    /// 以新的按鍵序列整個重算。`context` 是這串按鍵之前的最後一個詞。
    pub fn rebuild(&mut self, strokes: Vec<Stroke>, context: Option<u32>) {
        self.context = context.unwrap_or(NONE);
        self.reset();
        for stroke in strokes {
            self.push(stroke);
        }
    }

    /// 移除最後一個按鍵。
    pub fn pop(&mut self) {
        if self.strokes.pop().is_some() {
            self.partials.pop();
            self.nodes.pop();
        }
    }

    /// 丟掉最前面 `keys` 個按鍵（它們已經送出或釘住），結果和用剩下的按鍵 [`Lattice::rebuild`] 一樣。
    /// `context` 是被丟掉的那一段的最後一個詞。
    ///
    /// 按鍵有哪些讀法和前文無關，所以讀音樹上的進度可以留下來用，只有分數要重算，
    /// 比整個重算快得多。編輯區滿了之後每打一個字都會走到這裡。
    pub fn drop_prefix(&mut self, keys: usize, context: Option<u32>) {
        let shift = keys as u32;
        self.strokes.drain(..keys);
        let old = std::mem::take(&mut self.partials);
        // 留下從剩餘按鍵開始的進度。它們在清單裡的編號會變，`parent` 要跟著改。
        let mut renumbered: Vec<Vec<u32>> = Vec::new();
        for list in old.into_iter().skip(keys) {
            let mut numbers = vec![NONE; list.len()];
            let mut kept = Vec::new();
            for (index, partial) in list.into_iter().enumerate() {
                if partial.start < shift {
                    continue;
                }
                numbers[index] = kept.len() as u32;
                let syllable_start = partial.syllable_start - shift;
                let parent = match partial.parent {
                    NONE => NONE,
                    parent => renumbered[syllable_start as usize][parent as usize],
                };
                kept.push(Partial { start: partial.start - shift, syllable_start, parent, ..partial });
            }
            renumbered.push(numbers);
            self.partials.push(kept);
        }
        self.context = context.unwrap_or(NONE);
        self.nodes.clear();
        self.nodes.push(vec![self.start_node()]);
        for end in 1..=self.strokes.len() {
            let nodes = self.nodes_at(end);
            self.nodes.push(nodes);
        }
    }

    /// 加入一個按鍵，算出所有結束在新位置的解讀。
    pub fn push(&mut self, stroke: Stroke) {
        self.strokes.push(stroke);
        let end = self.strokes.len();
        let partials = self.partials_at(end);
        self.partials.push(partials);
        let nodes = self.nodes_at(end);
        self.nodes.push(nodes);
    }

    /// 讀音樹上所有「結束在位置 `end`」的進度。只看按鍵，和前文、分數都無關。
    fn partials_at(&self, end: usize) -> Vec<Partial> {
        let mut partials: Vec<Partial> = Vec::new();
        if matches!(self.strokes[end - 1], Stroke::Literal(_)) {
            return partials;
        }
        // 把最後幾個按鍵當成一個音節，接到讀音樹的進度上。有聲調的音節是 2 到 4 個按鍵；
        // 允許不打聲調時，再把最後 1 到 3 個按鍵當成沒有聲調的音節試一次。
        let shapes = [
            (true, MIN_SYLLABLE_KEYS..=MAX_SYLLABLE_KEYS),
            (false, MIN_TONELESS_KEYS..=MAX_TONELESS_KEYS),
        ];
        let mut syllables = Vec::new();
        let mut extendable: Vec<(usize, &Partial)> = Vec::new();
        for (has_tone, lengths) in shapes {
            if !has_tone && self.config.toneless_penalty.is_none() {
                continue;
            }
            let missing_tone = !has_tone as u8;
            for len in lengths {
                if len > end || matches!(self.strokes[end - len], Stroke::Literal(_)) {
                    break;
                }
                let start = end - len;
                syllables.clear();
                self.interpret(start, end, has_tone, &mut syllables);
                // 大部分的進度已經讀到詞的結尾，後面接不了任何音節，先濾掉。
                let earlier = self.partials[start].iter().enumerate();
                extendable.clear();
                extendable.extend(earlier.filter(|(_, p)| self.model.has_children(p.node)));
                for &(syllable, typos) in &syllables {
                    for &node in self.model.children(0, syllable) {
                        partials.push(Partial {
                            start: start as u32,
                            syllable_start: start as u32,
                            parent: NONE,
                            node,
                            typos,
                            toneless: missing_tone,
                            open: !has_tone,
                        });
                    }
                    for &(index, earlier) in &extendable {
                        let total = earlier.typos + typos;
                        if total > self.config.max_typos_per_word {
                            continue;
                        }
                        for &node in self.model.children(earlier.node, syllable) {
                            partials.push(Partial {
                                start: earlier.start,
                                syllable_start: start as u32,
                                parent: index as u32,
                                node,
                                typos: total,
                                toneless: earlier.toneless + missing_tone,
                                open: !has_tone,
                            });
                        }
                    }
                }
            }
        }
        partials
    }

    /// 所有「結束在位置 `end`」的路徑，分數高的排前面，最多 `beam` 條。
    /// 位置 `end` 的進度和它之前的路徑都要先算好。
    fn nodes_at(&self, end: usize) -> Vec<Node> {
        let stroke = self.strokes[end - 1];
        let mut nodes: Vec<Node> = Vec::new();

        // 只吃掉這一個按鍵的解讀：原樣字元，或猜不出來的注音鍵。
        let previous = &self.nodes[end - 1];
        // 使用者打了聲調，前一個音節就不該被當成沒打聲調；後面接著韻母的聲母也不該自己成一個字。
        // 所以猜不出來的按鍵如果看起來還屬於前一個音節，就不接在「最後一個音節沒打聲調」
        // 的詞後面，除非沒有別的路徑可以接。
        let continues = self.continues_syllable(end - 1);
        let open = |node: &Node| {
            node.kind == NodeKind::Word && self.partials[end - 1][node.partial as usize].open
        };
        let back = previous.iter().position(|node| !(continues && open(node))).unwrap_or(0);
        if let Some(best) = previous.get(back) {
            let (kind, penalty) = match stroke {
                Stroke::Literal(_) => (NodeKind::Literal, 0.0),
                Stroke::Zhuyin(_) => (NodeKind::Raw, self.config.raw_penalty),
            };
            nodes.push(Node {
                start: end as u32 - 1,
                back: back as u32,
                kind,
                word: NONE,
                partial: NONE,
                score: best.score + penalty,
            });
        }

        // 每個讀完的進度對應到一批詞，各自接上分數最高的前一段。
        for (index, partial) in self.partials[end].iter().enumerate() {
            let previous = &self.nodes[partial.start as usize];
            let entries = self.model.entries(partial.node);
            if previous.is_empty() || entries.is_empty() {
                continue;
            }
            let limit = if partial.typos == 0 && partial.toneless == 0 {
                self.config.words_per_span
            } else {
                self.config.words_per_typo_span
            };
            let cost =
                self.config.typo_penalty * partial.typos as f32 + self.toneless_cost(partial);
            for entry in entries.iter().take(limit) {
                let ceiling = self.model.best_score(entry.word);
                let mut best = (0u32, f32::NEG_INFINITY);
                for (i, node) in previous.iter().enumerate() {
                    // 前面的路徑是照分數由高到低排的：這一條就算拿到這個詞的最高分也贏不了，
                    // 後面的更不可能。
                    if node.score + ceiling <= best.1 {
                        break;
                    }
                    let context = (node.word != NONE).then_some(node.word);
                    let score = node.score + self.model.score(context, entry.word);
                    if score > best.1 {
                        best = (i as u32, score);
                    }
                }
                nodes.push(Node {
                    start: partial.start,
                    back: best.0,
                    kind: NodeKind::Word,
                    word: entry.word,
                    partial: index as u32,
                    score: best.1 + entry.penalty + cost,
                });
            }
        }

        // 分數高的排前面；同一段、同一個詞只留分數最高的那個。
        // 只需要前面 `beam` 個，湊滿就停，不必把後面幾百個都比對過。
        nodes.sort_by(|a, b| b.score.total_cmp(&a.score));
        let mut seen = HashSet::new();
        let mut kept = Vec::with_capacity(self.config.beam.min(nodes.len()));
        for node in nodes {
            if kept.len() == self.config.beam {
                break;
            }
            if seen.insert((node.start, node.kind as u8, node.word)) {
                kept.push(node);
            }
        }
        kept
    }

    /// 第 `at` 個按鍵照字面看起來還屬於前一個按鍵的音節：它是聲調，
    /// 或是排在前一個符號後面的介音、韻母。
    fn continues_syllable(&self, at: usize) -> bool {
        let slot = |at: usize| match self.strokes.get(at) {
            Some(Stroke::Zhuyin(key)) => self.layout.symbol(*key).map(|symbol| symbol.slot),
            _ => None,
        };
        match (at.checked_sub(1).and_then(slot), slot(at)) {
            (Some(last), Some(next)) => next > last,
            _ => false,
        }
    }

    /// 這個進度的最後一個音節沒打聲調，而且緊接在後面的按鍵看起來還屬於同一個音節。
    /// 整句猜測仍然會考慮這種切法（例如 ㄒㄧㄢ 可以是「西安」），但不拿來當候選字：
    /// 選了它會把一個音節從中間切開，留下孤立的聲調或韻母。
    /// 使用者真的要在這裡斷開時，補上聲調就可以。
    fn ends_mid_syllable(&self, partial: &Partial, end: usize) -> bool {
        partial.open && self.continues_syllable(end)
    }

    /// 這個進度裡沒打聲調的音節一共扣幾分。
    fn toneless_cost(&self, partial: &Partial) -> f32 {
        self.config.toneless_penalty.unwrap_or(0.0) * partial.toneless as f32
    }

    /// 把 `start..end` 的按鍵當成一個音節，列出所有可能的讀法與各自的誤觸次數。
    /// `has_tone` 是 `false` 時，這些按鍵裡沒有聲調鍵，列出來的是聲調不拘的音節。
    fn interpret(&self, start: usize, end: usize, has_tone: bool, out: &mut Vec<(Syllable, u8)>) {
        let config = &self.config;
        let max_typos = if has_tone {
            config.max_typos_per_syllable
        } else if end - start == 1 {
            // 只有一個符號又沒有聲調的音節不容錯：它的相鄰鍵幾乎都能自己成一個字，
            // 容錯會讓每個按鍵多出一大批解讀，卻很少真的救回什麼。
            0
        } else {
            // 另有上限，但不會超過一般音節的上限，所以關閉容錯只要改一個設定。
            config.max_typos_per_toneless_syllable.min(config.max_typos_per_syllable)
        };
        self.interpret_from(start, end, has_tone, max_typos, [0; 4], 0, out);
        // 順序不拘時，不同的誤觸組合可能湊出同一個音節，只留誤觸最少的那個。
        out.sort_by_key(|&(syllable, typos)| (syllable, typos));
        out.dedup_by_key(|&mut (syllable, _)| syllable);
    }

    fn interpret_from(
        &self,
        at: usize,
        end: usize,
        has_tone: bool,
        max_typos: u8,
        parts: [u8; 4],
        typos: u8,
        out: &mut Vec<(Syllable, u8)>,
    ) {
        if at == end {
            // 有聲調鍵時聲調已經確定；沒有時聲調不拘，之後查讀音樹時每個聲調都會查到。
            let syllable = if has_tone { Syllable::new(parts) } else { Syllable::any_tone(parts) };
            if let Some(syllable) = syllable.filter(|s| self.model.is_valid_syllable(*s)) {
                out.push((syllable, typos));
            }
            return;
        }
        let Stroke::Zhuyin(key) = self.strokes[at] else { return };
        let expects_tone = has_tone && at + 1 == end;
        let mut consider = |key: char, typos: u8| {
            let Some(symbol) = self.layout.symbol(key) else { return };
            // 只有最後一個鍵是聲調。聲調之前的聲母、介音、韻母不管按的順序，
            // 因為兩手幾乎同時按下時順序常常顛倒；但同一個位置不能有兩個符號。
            if (symbol.slot == Slot::Tone) != expects_tone || parts[symbol.slot as usize] != 0 {
                return;
            }
            // 沒有聲調鍵標出字的結尾時，順序顛倒的按鍵很容易被誤認成跨越兩個字的另一種切法，
            // 所以沒打聲調的音節必須照聲母、介音、韻母的順序。
            let out_of_order = parts[symbol.slot as usize..].iter().any(|&index| index != 0);
            if !has_tone && out_of_order {
                return;
            }
            let mut next = parts;
            next[symbol.slot as usize] = symbol.index;
            self.interpret_from(at + 1, end, has_tone, max_typos, next, typos, out);
        };
        consider(key, typos);
        if typos < max_typos {
            for &neighbor in self.layout.neighbors(key) {
                consider(neighbor, typos + 1);
            }
        }
    }

    /// 目前分數最高的整句解讀，由前到後列出每一段。
    pub fn best_path(&self) -> Vec<Segment> {
        let mut segments = Vec::new();
        let mut end = self.strokes.len();
        let Some(mut node) = self.nodes[end].first().copied() else {
            return segments;
        };
        while node.kind != NodeKind::Start {
            let start = node.start as usize;
            let mut syllable_starts = Vec::new();
            let mut syllable_typos = Vec::new();
            if node.kind == NodeKind::Word {
                let mut partial = self.partials[end][node.partial as usize];
                loop {
                    syllable_starts.push(partial.syllable_start as usize);
                    if partial.parent == NONE {
                        syllable_typos.push(partial.typos);
                        break;
                    }
                    // 誤觸次數是從詞的開頭累計的，相減才是這個音節自己的。
                    let parent =
                        self.partials[partial.syllable_start as usize][partial.parent as usize];
                    syllable_typos.push(partial.typos - parent.typos);
                    partial = parent;
                }
                syllable_starts.reverse();
                syllable_typos.reverse();
            }
            segments.push(Segment {
                start,
                end,
                kind: match node.kind {
                    NodeKind::Word => SegmentKind::Word,
                    NodeKind::Raw => SegmentKind::Raw,
                    _ => SegmentKind::Literal,
                },
                word: (node.word != NONE).then_some(node.word),
                syllable_starts,
                syllable_typos,
            });
            let previous = self.nodes[start][node.back as usize];
            end = start;
            node = previous;
        }
        segments.reverse();
        segments
    }

    /// 從位置 `start` 開始可以選的字詞。
    ///
    /// 排序：照字面讀法的多字詞最前面（長的優先），其餘依可能性排列，
    /// 所以照字面讀法的字通常排在誤觸讀法的字前面。誤觸讀法的數量很多，只留最可能的一批。
    pub fn candidates(&self, start: usize) -> Vec<Candidate> {
        let mut found: Vec<(Candidate, f32)> = Vec::new();
        for end in start + MIN_TONELESS_KEYS..=self.strokes.len() {
            for partial in self.partials[end].iter().filter(|p| p.start as usize == start) {
                if self.ends_mid_syllable(partial, end) {
                    continue;
                }
                for entry in self.model.entries(partial.node) {
                    let likelihood = self.model.unigram(entry.word)
                        + entry.penalty
                        + self.config.typo_penalty * partial.typos as f32
                        + self.toneless_cost(partial);
                    let candidate = Candidate { start, end, word: entry.word, typos: partial.typos };
                    found.push((candidate, likelihood));
                }
            }
        }
        // 同一個詞可能由好幾種讀法得到，留可能性最高的那個。
        found.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.word.cmp(&b.0.word)));
        let mut seen = HashSet::new();
        found.retain(|(c, _)| seen.insert((c.end, c.word)));

        let mut typo_readings = 0;
        found.retain(|(c, _)| {
            typo_readings += (c.typos > 0) as usize;
            c.typos == 0 || typo_readings <= MAX_TYPO_CANDIDATES
        });
        let length = |c: &Candidate| self.model.word(c.word).chars().count();
        let exact_word = |c: &Candidate| c.typos == 0 && length(c) > 1;
        // 排序是穩定的，所以同一組裡仍然照可能性排列。
        found.sort_by(|(a, _), (b, _)| {
            exact_word(b).cmp(&exact_word(a)).then_with(|| {
                if exact_word(a) { length(b).cmp(&length(a)) } else { std::cmp::Ordering::Equal }
            })
        });
        found.into_iter().map(|(candidate, _)| candidate).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelBuilder;

    fn lattice() -> Lattice {
        let words = [
            ("一", "ㄧ", -2.0),
            ("本", "ㄅㄣˇ", -3.0),
            ("手", "ㄕㄡˇ", -3.3),
            ("冊", "ㄘㄜˋ", -4.6),
            ("測", "ㄘㄜˋ", -3.6),
            ("手冊", "ㄕㄡˇ ㄘㄜˋ", -4.8),
            ("測試", "ㄘㄜˋ ㄕˋ", -3.9),
            ("是", "ㄕˋ", -2.0),
            ("我", "ㄨㄛˇ", -2.2),
        ];
        let mut builder = ModelBuilder::new();
        for (word, reading, probability) in words {
            assert!(builder.add_entry(word, reading, 0.0));
            builder.set_unigram(word, probability);
        }
        builder.add_bigram("本", "手冊", -0.8);
        builder.add_bigram("我", "是", -1.0);
        let model = Model::from_bytes(&builder.build()).unwrap();
        Lattice::new(Arc::new(model), Layout::dachen(), Config::default())
    }

    #[test]
    fn dropping_a_prefix_gives_the_same_lattice_as_rebuilding_the_rest() {
        // 有聲調、沒聲調、誤觸（hl4、g,）、猜不出來的按鍵（11）和原樣字元（|）都混在一起。
        let strokes: Vec<Stroke> = "u 1p3g.hk4|ji3g,11hl4g4hkg"
            .chars()
            .map(|key| if key == '|' { Stroke::Literal('，') } else { Stroke::Zhuyin(key) })
            .collect();
        for cut in 0..=strokes.len() {
            for context in [None, Some(0)] {
                let mut dropped = lattice();
                for &stroke in &strokes {
                    dropped.push(stroke);
                }
                dropped.drop_prefix(cut, context);
                let mut rebuilt = lattice();
                rebuilt.rebuild(strokes[cut..].to_vec(), context);
                assert_eq!(dropped.strokes, rebuilt.strokes, "丟掉 {cut} 個按鍵");
                assert_eq!(dropped.partials, rebuilt.partials, "丟掉 {cut} 個按鍵");
                assert_eq!(dropped.nodes, rebuilt.nodes, "丟掉 {cut} 個按鍵");
            }
        }
    }
}
