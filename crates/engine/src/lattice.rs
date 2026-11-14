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

/// 影響猜測結果的參數。分數都是 log10 機率，越接近 0 越好。
#[derive(Clone, Debug)]
pub struct Config {
    /// 每一次誤觸的扣分。
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
}

impl Default for Config {
    fn default() -> Config {
        Config {
            typo_penalty: -3.0,
            max_typos_per_syllable: 0,
            max_typos_per_word: 3,
            raw_penalty: -20.0,
            words_per_span: 40,
            words_per_typo_span: 10,
            beam: 80,
            max_cells: 50,
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
    pub pinned: bool,
    /// 這一段裡每個音節從哪個位置開始（只有 `Word` 有意義）。
    pub syllable_starts: Vec<usize>,
}

/// 使用者手動選定的字詞：`start..end` 這段按鍵一定要解讀成 `word`。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pin {
    pub start: usize,
    pub end: usize,
    pub word: u32,
}

/// 某個位置可以選的一個候選字詞。
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub start: usize,
    pub end: usize,
    pub word: u32,
    pub typos: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeKind {
    Start,
    Word,
    Raw,
    Literal,
}

/// 一條「結束在某個位置」的路徑的最後一段。
#[derive(Clone, Copy)]
struct Node {
    start: u32,
    /// 前一段是 `nodes[start]` 裡的第幾個。
    back: u32,
    kind: NodeKind,
    word: u32,
    /// 對應 `partials[結束位置]` 裡的第幾個，用來找回音節的邊界。
    partial: u32,
    score: f32,
    pinned: bool,
}

/// 讀音樹上走到一半的進度：從 `start` 開始的按鍵，目前讀到 `node`。
#[derive(Clone, Copy)]
struct Partial {
    start: u32,
    /// 最後一個音節從哪裡開始。
    syllable_start: u32,
    /// 前一個音節的進度是 `partials[syllable_start]` 裡的第幾個；第一個音節是 `NONE`。
    parent: u32,
    node: u32,
    typos: u8,
}

pub struct Lattice {
    model: Arc<Model>,
    layout: Layout,
    config: Config,
    strokes: Vec<Stroke>,
    partials: Vec<Vec<Partial>>,
    nodes: Vec<Vec<Node>>,
    pins: Vec<Pin>,
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
            pins: Vec::new(),
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

    pub fn pins(&self) -> &[Pin] {
        &self.pins
    }

    fn reset(&mut self) {
        self.strokes.clear();
        self.partials = vec![Vec::new()];
        self.nodes = vec![vec![Node {
            start: 0,
            back: NONE,
            kind: NodeKind::Start,
            word: NONE,
            partial: NONE,
            score: 0.0,
            pinned: false,
        }]];
    }

    /// 清空按鍵序列與所有釘住的位置。
    pub fn clear(&mut self) {
        self.pins.clear();
        self.reset();
    }

    /// 以新的按鍵序列與釘住位置整個重算。
    pub fn rebuild(&mut self, strokes: Vec<Stroke>, pins: Vec<Pin>) {
        self.reset();
        self.pins = pins;
        for stroke in strokes {
            self.push(stroke);
        }
    }

    /// 移除最後一個按鍵。
    pub fn pop(&mut self) {
        if self.strokes.pop().is_some() {
            self.partials.pop();
            self.nodes.pop();
            let len = self.strokes.len();
            self.pins.retain(|pin| pin.end <= len);
        }
    }

    /// 釘住一段按鍵；和它重疊的舊釘子會被取代。
    pub fn pin(&mut self, pin: Pin) {
        let mut pins = std::mem::take(&mut self.pins);
        pins.retain(|old| old.end <= pin.start || old.start >= pin.end);
        pins.push(pin);
        let strokes = std::mem::take(&mut self.strokes);
        self.rebuild(strokes, pins);
    }

    /// `start..end` 剛好是某個釘子時，回傳被釘住的詞。
    fn pinned_word(&self, start: usize, end: usize) -> Option<u32> {
        self.pins.iter().find(|p| p.start == start && p.end == end).map(|p| p.word)
    }

    fn overlaps_pin(&self, start: usize, end: usize) -> bool {
        self.pins.iter().any(|p| p.start < end && start < p.end)
    }

    /// 加入一個按鍵，算出所有結束在新位置的解讀。
    pub fn push(&mut self, stroke: Stroke) {
        self.strokes.push(stroke);
        let end = self.strokes.len();
        let mut partials: Vec<Partial> = Vec::new();
        let mut nodes: Vec<Node> = Vec::new();

        // 只吃掉這一個按鍵的解讀：原樣字元，或猜不出來的注音鍵。
        if !self.overlaps_pin(end - 1, end) {
            if let Some(best) = self.nodes[end - 1].first() {
                let (kind, penalty) = match stroke {
                    Stroke::Literal(_) => (NodeKind::Literal, 0.0),
                    Stroke::Zhuyin(_) => (NodeKind::Raw, self.config.raw_penalty),
                };
                nodes.push(Node {
                    start: end as u32 - 1,
                    back: 0,
                    kind,
                    word: NONE,
                    partial: NONE,
                    score: best.score + penalty,
                    pinned: false,
                });
            }
        }

        if matches!(stroke, Stroke::Zhuyin(_)) {
            // 把最後 2 到 4 個按鍵當成一個音節，接到讀音樹的進度上。
            let mut syllables = Vec::new();
            for len in MIN_SYLLABLE_KEYS..=MAX_SYLLABLE_KEYS {
                if len > end || matches!(self.strokes[end - len], Stroke::Literal(_)) {
                    break;
                }
                let start = end - len;
                syllables.clear();
                self.interpret(start, end, &mut syllables);
                for &(syllable, typos) in &syllables {
                    if let Some(node) = self.model.child(0, syllable) {
                        partials.push(Partial {
                            start: start as u32,
                            syllable_start: start as u32,
                            parent: NONE,
                            node,
                            typos,
                        });
                    }
                    for (index, earlier) in self.partials[start].iter().enumerate() {
                        let total = earlier.typos + typos;
                        if total > self.config.max_typos_per_word {
                            continue;
                        }
                        if let Some(node) = self.model.child(earlier.node, syllable) {
                            partials.push(Partial {
                                start: earlier.start,
                                syllable_start: start as u32,
                                parent: index as u32,
                                node,
                                typos: total,
                            });
                        }
                    }
                }
            }

            // 每個讀完的進度對應到一批詞，各自接上分數最高的前一段。
            for (index, partial) in partials.iter().enumerate() {
                let start = partial.start as usize;
                let previous = &self.nodes[start];
                let entries = self.model.entries(partial.node);
                if previous.is_empty() || entries.is_empty() {
                    continue;
                }
                let pinned = self.pinned_word(start, end);
                if pinned.is_none() && self.overlaps_pin(start, end) {
                    continue;
                }
                let limit = if pinned.is_some() {
                    entries.len()
                } else if partial.typos == 0 {
                    self.config.words_per_span
                } else {
                    self.config.words_per_typo_span
                };
                let typo_cost = self.config.typo_penalty * partial.typos as f32;
                for entry in entries.iter().take(limit) {
                    if pinned.is_some_and(|word| word != entry.word) {
                        continue;
                    }
                    let mut best = (0u32, f32::NEG_INFINITY);
                    for (i, node) in previous.iter().enumerate() {
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
                        score: best.1 + entry.penalty + typo_cost,
                        pinned: pinned.is_some(),
                    });
                }
            }
        }

        // 分數高的排前面；同一段、同一個詞只留分數最高的那個。
        nodes.sort_by(|a, b| b.score.total_cmp(&a.score));
        let mut seen = HashSet::new();
        nodes.retain(|n| seen.insert((n.start, n.kind as u8, n.word)));
        nodes.truncate(self.config.beam);

        self.partials.push(partials);
        self.nodes.push(nodes);
    }

    /// 把 `start..end` 的按鍵當成一個音節，列出所有可能的讀法與各自的誤觸次數。
    fn interpret(&self, start: usize, end: usize, out: &mut Vec<(Syllable, u8)>) {
        self.interpret_from(start, end, -1, [0; 4], 0, out);
    }

    fn interpret_from(
        &self,
        at: usize,
        end: usize,
        last_slot: i8,
        parts: [u8; 4],
        typos: u8,
        out: &mut Vec<(Syllable, u8)>,
    ) {
        if at == end {
            if let Some(syllable) = Syllable::new(parts) {
                if self.model.is_valid_syllable(syllable) {
                    out.push((syllable, typos));
                }
            }
            return;
        }
        let Stroke::Zhuyin(key) = self.strokes[at] else { return };
        let is_last = at + 1 == end;
        let mut consider = |key: char, typos: u8| {
            let Some(symbol) = self.layout.symbol(key) else { return };
            // 符號要依聲母、介音、韻母的順序出現，而且只有最後一個鍵是聲調。
            if (symbol.slot == Slot::Tone) != is_last || symbol.slot as i8 <= last_slot {
                return;
            }
            let mut next = parts;
            next[symbol.slot as usize] = symbol.index;
            self.interpret_from(at + 1, end, symbol.slot as i8, next, typos, out);
        };
        consider(key, typos);
        if typos < self.config.max_typos_per_syllable {
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
            if node.kind == NodeKind::Word {
                let mut partial = self.partials[end][node.partial as usize];
                loop {
                    syllable_starts.push(partial.syllable_start as usize);
                    if partial.parent == NONE {
                        break;
                    }
                    partial = self.partials[partial.syllable_start as usize][partial.parent as usize];
                }
                syllable_starts.reverse();
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
                pinned: node.pinned,
                syllable_starts,
            });
            let previous = self.nodes[start][node.back as usize];
            end = start;
            node = previous;
        }
        segments.reverse();
        segments
    }

    /// 從位置 `start` 開始可以選的所有字詞。
    /// 沒有誤觸的排前面，其次是比較長的詞，再來是比較常見的詞。
    pub fn candidates(&self, start: usize) -> Vec<Candidate> {
        let mut found: Vec<Candidate> = Vec::new();
        for end in start + MIN_SYLLABLE_KEYS..=self.strokes.len() {
            for partial in self.partials[end].iter().filter(|p| p.start as usize == start) {
                for entry in self.model.entries(partial.node) {
                    found.push(Candidate { start, end, word: entry.word, typos: partial.typos });
                }
            }
        }
        let length = |c: &Candidate| self.model.word(c.word).chars().count();
        found.sort_by(|a, b| {
            (a.typos > 0)
                .cmp(&(b.typos > 0))
                .then(length(b).cmp(&length(a)))
                .then(a.typos.cmp(&b.typos))
                .then(self.model.unigram(b.word).total_cmp(&self.model.unigram(a.word)))
                .then(a.word.cmp(&b.word))
        });
        let mut seen = HashSet::new();
        found.retain(|c| seen.insert((c.end, c.word)));
        found
    }
}
