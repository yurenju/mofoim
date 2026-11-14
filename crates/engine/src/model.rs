//! 詞庫與語言模型：資料檔的格式、讀取，以及建立資料檔的 `ModelBuilder`。
//!
//! 引擎本身不讀檔，前端把資料檔的位元組交給 `Model::from_bytes`。

use crate::syllable::Syllable;
use std::collections::{BTreeSet, HashMap};

const MAGIC: &[u8; 8] = b"MOFOIM\x00\x01";

/// 有 bigram 時，bigram 機率占的比重；其餘比重給單一詞的機率。
const BIGRAM_WEIGHT: f32 = 0.8;

/// 詞庫裡的一筆：某個讀音可以對應到的一個詞。
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// 詞的編號。
    pub word: u32,
    /// 這個讀音額外的扣分（例如罕用讀音），以 log10 表示，0 代表不扣分。
    pub penalty: f32,
}

pub struct Model {
    word_offsets: Vec<u32>,
    word_blob: String,
    unigram: Vec<f32>,
    // 讀音樹：節點 0 是根，從根沿著音節往下走到的節點，代表那串讀音。
    node_child_start: Vec<u32>,
    node_entry_start: Vec<u32>,
    child_syllable: Vec<u16>,
    child_node: Vec<u32>,
    entries: Vec<Entry>,
    // 每個詞後面可以接哪些詞：bigram_start[w]..bigram_start[w+1] 是詞 w 的範圍。
    bigram_start: Vec<u32>,
    bigram_next: Vec<u32>,
    bigram_score: Vec<f32>,
    valid_syllable: Vec<bool>,
    /// 每個詞在 [`Model::score`] 可能拿到的最高分。
    best_score: Vec<f32>,
}

impl Model {
    pub fn from_bytes(bytes: &[u8]) -> Result<Model, String> {
        if bytes.len() < MAGIC.len() || &bytes[..MAGIC.len()] != MAGIC {
            return Err("不是 mofoim 的資料檔，或版本不符".into());
        }
        let mut reader = Reader { bytes, pos: MAGIC.len() };
        let word_offsets = reader.u32s()?;
        let word_blob = String::from_utf8(reader.section()?.to_vec())
            .map_err(|_| "資料檔裡的字詞不是合法的 UTF-8".to_string())?;
        let unigram = reader.f32s()?;
        let node_child_start = reader.u32s()?;
        let node_entry_start = reader.u32s()?;
        let child_syllable = reader.u16s()?;
        let child_node = reader.u32s()?;
        let entry_word = reader.u32s()?;
        let entry_penalty = reader.f32s()?;
        let bigram_start = reader.u32s()?;
        let bigram_next = reader.u32s()?;
        let bigram_score = reader.f32s()?;

        let words = unigram.len();
        let consistent = word_offsets.len() == words + 1
            && bigram_start.len() == words + 1
            && node_child_start.len() == node_entry_start.len()
            && !node_child_start.is_empty()
            && child_syllable.len() == child_node.len()
            && entry_word.len() == entry_penalty.len()
            && bigram_next.len() == bigram_score.len();
        if !consistent {
            return Err("資料檔內容不一致，可能已損毀".into());
        }

        let mut valid_syllable = vec![false; Syllable::LIMIT];
        for &syllable in &child_syllable {
            valid_syllable[syllable as usize] = true;
            // 聲調不拘的音節（聲調的位元是 0）：只要有任何一個聲調用到就算。
            valid_syllable[(syllable & !7) as usize] = true;
        }
        if bigram_next.iter().any(|&word| word as usize >= words) {
            return Err("資料檔內容不一致，可能已損毀".into());
        }
        let backoff = (1.0 - BIGRAM_WEIGHT).log10();
        let mut best_score: Vec<f32> = unigram.iter().map(|score| score + backoff).collect();
        for (&word, &score) in bigram_next.iter().zip(&bigram_score) {
            let best = &mut best_score[word as usize];
            *best = best.max(score);
        }
        let entries = entry_word
            .iter()
            .zip(&entry_penalty)
            .map(|(&word, &penalty)| Entry { word, penalty })
            .collect();
        Ok(Model {
            word_offsets,
            word_blob,
            unigram,
            node_child_start,
            node_entry_start,
            child_syllable,
            child_node,
            entries,
            bigram_start,
            bigram_next,
            bigram_score,
            valid_syllable,
            best_score,
        })
    }

    pub fn word_count(&self) -> usize {
        self.unigram.len()
    }

    pub fn word(&self, id: u32) -> &str {
        let start = self.word_offsets[id as usize] as usize;
        let end = self.word_offsets[id as usize + 1] as usize;
        &self.word_blob[start..end]
    }

    /// 這個詞單獨出現的機率（log10）。
    pub fn unigram(&self, id: u32) -> f32 {
        self.unigram[id as usize]
    }

    /// 詞庫裡有沒有任何字詞用到這個音節。
    pub fn is_valid_syllable(&self, syllable: Syllable) -> bool {
        self.valid_syllable[syllable.0 as usize]
    }

    /// 從讀音樹的 `node` 沿著 `syllable` 往下走一步。根節點是 0。
    pub fn child(&self, node: u32, syllable: Syllable) -> Option<u32> {
        let start = self.node_child_start[node as usize] as usize;
        let end = self.node_child_start[node as usize + 1] as usize;
        let found = self.child_syllable[start..end].binary_search(&syllable.0).ok()?;
        Some(self.child_node[start + found])
    }

    /// 讀音樹的 `node` 底下還有沒有更長的讀音。
    pub fn has_children(&self, node: u32) -> bool {
        self.node_child_start[node as usize] < self.node_child_start[node as usize + 1]
    }

    /// 從讀音樹的 `node` 沿著 `syllable` 往下走一步可以到的所有節點。
    /// `syllable` 是聲調不拘的音節時，每個聲調各有一個節點；否則最多一個。
    pub fn children(&self, node: u32, syllable: Syllable) -> &[u32] {
        let start = self.node_child_start[node as usize] as usize;
        let end = self.node_child_start[node as usize + 1] as usize;
        let syllables = &self.child_syllable[start..end];
        // 子節點依音節的數值排序，而聲調在最低的三個位元，所以同一個音節的各個聲調是連在一起的。
        let (low, high) =
            if syllable.has_tone() { (syllable.0, syllable.0) } else { (syllable.0, syllable.0 | 7) };
        let from = syllables.partition_point(|&s| s < low);
        let to = from + syllables[from..].partition_point(|&s| s <= high);
        &self.child_node[start + from..start + to]
    }

    /// 讀音剛好走到 `node` 的所有詞，機率高的排前面。
    pub fn entries(&self, node: u32) -> &[Entry] {
        let start = self.node_entry_start[node as usize] as usize;
        let end = self.node_entry_start[node as usize + 1] as usize;
        &self.entries[start..end]
    }

    /// 前一個詞是 `previous` 時，接著出現 `word` 的機率（log10）。
    /// `previous` 是 `None` 代表句首，或前面不是詞庫裡的詞。
    pub fn score(&self, previous: Option<u32>, word: u32) -> f32 {
        if let Some(previous) = previous {
            let start = self.bigram_start[previous as usize] as usize;
            let end = self.bigram_start[previous as usize + 1] as usize;
            if let Ok(found) = self.bigram_next[start..end].binary_search(&word) {
                return self.bigram_score[start + found];
            }
        }
        self.unigram[word as usize] + (1.0 - BIGRAM_WEIGHT).log10()
    }

    /// 不管前一個詞是什麼，[`Model::score`] 對 `word` 可能回傳的最高分。
    /// 找分數最高的前文時，可以用它判斷剩下的前文還有沒有機會。
    pub fn best_score(&self, word: u32) -> f32 {
        self.best_score[word as usize]
    }

    /// 走訪詞庫裡的每一筆：詞、讀音、扣分。供工具使用，引擎猜句子時不會用到。
    pub fn for_each_entry(&self, mut visit: impl FnMut(&str, &[Syllable], f32)) {
        let mut stack: Vec<(u32, Vec<Syllable>)> = vec![(0, Vec::new())];
        while let Some((node, reading)) = stack.pop() {
            for entry in self.entries(node) {
                visit(self.word(entry.word), &reading, entry.penalty);
            }
            let start = self.node_child_start[node as usize] as usize;
            let end = self.node_child_start[node as usize + 1] as usize;
            for i in start..end {
                let mut next = reading.clone();
                next.push(Syllable(self.child_syllable[i]));
                stack.push((self.child_node[i], next));
            }
        }
    }

    /// 詞的編號；詞庫裡沒有就是 `None`。供工具與測試使用。
    pub fn word_id(&self, text: &str) -> Option<u32> {
        (0..self.word_count() as u32).find(|&id| self.word(id) == text)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn section(&mut self) -> Result<&'a [u8], String> {
        let truncated = || "資料檔不完整".to_string();
        let header = self.bytes.get(self.pos..self.pos + 4).ok_or_else(truncated)?;
        let len = u32::from_le_bytes(header.try_into().unwrap()) as usize;
        let start = self.pos + 4;
        let data = self.bytes.get(start..start + len).ok_or_else(truncated)?;
        self.pos = start + len;
        Ok(data)
    }

    fn u32s(&mut self) -> Result<Vec<u32>, String> {
        let data = self.section()?;
        Ok(data.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect())
    }

    fn u16s(&mut self) -> Result<Vec<u16>, String> {
        let data = self.section()?;
        Ok(data.chunks_exact(2).map(|c| u16::from_le_bytes(c.try_into().unwrap())).collect())
    }

    fn f32s(&mut self) -> Result<Vec<f32>, String> {
        Ok(self.u32s()?.into_iter().map(f32::from_bits).collect())
    }
}

fn write_section(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
}

fn write_u32s(out: &mut Vec<u8>, values: &[u32]) {
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    write_section(out, &bytes);
}

fn write_u16s(out: &mut Vec<u8>, values: &[u16]) {
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    write_section(out, &bytes);
}

fn write_f32s(out: &mut Vec<u8>, values: &[f32]) {
    let bits: Vec<u32> = values.iter().map(|v| v.to_bits()).collect();
    write_u32s(out, &bits);
}

/// 收集詞庫與語言模型的原始資料，產生資料檔的位元組。
#[derive(Default)]
pub struct ModelBuilder {
    entries: Vec<(String, Vec<Syllable>, f32)>,
    unigrams: HashMap<String, f32>,
    bigrams: Vec<(String, String, f32)>,
    /// 語言模型裡沒有的詞，用這個機率（log10）。
    pub unknown_unigram: f32,
}

impl ModelBuilder {
    pub fn new() -> ModelBuilder {
        ModelBuilder { unknown_unigram: -8.0, ..Default::default() }
    }

    /// 加入一筆詞與讀音。`reading` 是用空白隔開的音節，例如「ㄘㄜˋ ㄕˋ」。
    /// 讀音無法解析，或音節數與字數不同時回傳 `false`，不會加入。
    pub fn add_entry(&mut self, word: &str, reading: &str, penalty: f32) -> bool {
        let syllables: Option<Vec<Syllable>> =
            reading.split_whitespace().map(Syllable::parse).collect();
        match syllables {
            Some(s) if !s.is_empty() && s.len() == word.chars().count() => {
                self.entries.push((word.to_string(), s, penalty));
                true
            }
            _ => false,
        }
    }

    /// 設定詞單獨出現的機率（log10）；同一個詞只有第一次設定有效。
    pub fn set_unigram(&mut self, word: &str, log10_probability: f32) {
        self.unigrams.entry(word.to_string()).or_insert(log10_probability);
    }

    /// 加入「`first` 後面接 `second`」的機率（log10）。
    pub fn add_bigram(&mut self, first: &str, second: &str, log10_probability: f32) {
        self.bigrams.push((first.to_string(), second.to_string(), log10_probability));
    }

    pub fn build(&self) -> Vec<u8> {
        // 詞的編號：依字串排序。
        let words: Vec<&str> = self
            .entries
            .iter()
            .map(|(word, _, _)| word.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let ids: HashMap<&str, u32> =
            words.iter().enumerate().map(|(i, w)| (*w, i as u32)).collect();
        let unigram: Vec<f32> = words
            .iter()
            .map(|w| self.unigrams.get(*w).copied().unwrap_or(self.unknown_unigram))
            .collect();

        // 讀音樹的節點：每一個讀音的每一段開頭都是一個節點。
        let mut node_ids: HashMap<Vec<u16>, u32> = HashMap::new();
        node_ids.insert(Vec::new(), 0);
        let mut links: Vec<(u32, u16, u32)> = Vec::new(); // 父節點、音節、子節點
        let mut node_entries: Vec<Vec<(u32, f32)>> = vec![Vec::new()];
        for (word, syllables, penalty) in &self.entries {
            let mut node = 0u32;
            let mut prefix: Vec<u16> = Vec::new();
            for syllable in syllables {
                prefix.push(syllable.0);
                node = match node_ids.get(&prefix) {
                    Some(&existing) => existing,
                    None => {
                        let created = node_entries.len() as u32;
                        node_ids.insert(prefix.clone(), created);
                        node_entries.push(Vec::new());
                        links.push((node, syllable.0, created));
                        created
                    }
                };
            }
            let entry = (ids[word.as_str()], *penalty);
            if !node_entries[node as usize].iter().any(|e| e.0 == entry.0) {
                node_entries[node as usize].push(entry);
            }
        }
        links.sort();

        let node_count = node_entries.len();
        let mut node_child_start = vec![0u32; node_count + 1];
        for &(parent, _, _) in &links {
            node_child_start[parent as usize + 1] += 1;
        }
        for i in 0..node_count {
            node_child_start[i + 1] += node_child_start[i];
        }
        let child_syllable: Vec<u16> = links.iter().map(|l| l.1).collect();
        let child_node: Vec<u32> = links.iter().map(|l| l.2).collect();

        let mut node_entry_start = Vec::with_capacity(node_count + 1);
        let mut entry_word = Vec::new();
        let mut entry_penalty = Vec::new();
        for list in &mut node_entries {
            node_entry_start.push(entry_word.len() as u32);
            let rank = |e: &(u32, f32)| unigram[e.0 as usize] + e.1;
            list.sort_by(|a, b| rank(b).total_cmp(&rank(a)).then(a.0.cmp(&b.0)));
            for &(word, penalty) in list.iter() {
                entry_word.push(word);
                entry_penalty.push(penalty);
            }
        }
        node_entry_start.push(entry_word.len() as u32);

        // bigram：先把機率和單一詞的機率混合好，查詢時就不用再算。
        let mut pairs: Vec<(u32, u32, f32)> = self
            .bigrams
            .iter()
            .filter_map(|(first, second, lp)| {
                let a = *ids.get(first.as_str())?;
                let b = *ids.get(second.as_str())?;
                let mixed = BIGRAM_WEIGHT * 10f32.powf(*lp)
                    + (1.0 - BIGRAM_WEIGHT) * 10f32.powf(unigram[b as usize]);
                Some((a, b, mixed.log10()))
            })
            .collect();
        pairs.sort_by(|x, y| (x.0, x.1).cmp(&(y.0, y.1)));
        pairs.dedup_by(|later, earlier| (later.0, later.1) == (earlier.0, earlier.1));
        let mut bigram_start = vec![0u32; words.len() + 1];
        for &(first, _, _) in &pairs {
            bigram_start[first as usize + 1] += 1;
        }
        for i in 0..words.len() {
            bigram_start[i + 1] += bigram_start[i];
        }
        let bigram_next: Vec<u32> = pairs.iter().map(|p| p.1).collect();
        let bigram_score: Vec<f32> = pairs.iter().map(|p| p.2).collect();

        let mut word_offsets = Vec::with_capacity(words.len() + 1);
        let mut word_blob = String::new();
        for word in &words {
            word_offsets.push(word_blob.len() as u32);
            word_blob.push_str(word);
        }
        word_offsets.push(word_blob.len() as u32);

        let mut out = MAGIC.to_vec();
        write_u32s(&mut out, &word_offsets);
        write_section(&mut out, word_blob.as_bytes());
        write_f32s(&mut out, &unigram);
        write_u32s(&mut out, &node_child_start);
        write_u32s(&mut out, &node_entry_start);
        write_u16s(&mut out, &child_syllable);
        write_u32s(&mut out, &child_node);
        write_u32s(&mut out, &entry_word);
        write_f32s(&mut out, &entry_penalty);
        write_u32s(&mut out, &bigram_start);
        write_u32s(&mut out, &bigram_next);
        write_f32s(&mut out, &bigram_score);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Model {
        let mut builder = ModelBuilder::new();
        assert!(builder.add_entry("冊", "ㄘㄜˋ", 0.0));
        assert!(builder.add_entry("測", "ㄘㄜˋ", 0.0));
        assert!(builder.add_entry("試", "ㄕˋ", 0.0));
        assert!(builder.add_entry("測試", "ㄘㄜˋ ㄕˋ", 0.0));
        builder.set_unigram("冊", -4.0);
        builder.set_unigram("測", -3.0);
        builder.set_unigram("試", -3.5);
        builder.set_unigram("測試", -3.2);
        builder.add_bigram("測", "試", -1.0);
        builder.add_bigram("測", "不存在", -1.0);
        Model::from_bytes(&builder.build()).unwrap()
    }

    fn syllable(text: &str) -> Syllable {
        Syllable::parse(text).unwrap()
    }

    #[test]
    fn looks_up_words_by_reading_most_likely_first() {
        let model = sample();
        let node = model.child(0, syllable("ㄘㄜˋ")).unwrap();
        let words: Vec<&str> = model.entries(node).iter().map(|e| model.word(e.word)).collect();
        assert_eq!(words, ["測", "冊"]);
    }

    #[test]
    fn walks_multi_syllable_readings() {
        let model = sample();
        let first = model.child(0, syllable("ㄘㄜˋ")).unwrap();
        let second = model.child(first, syllable("ㄕˋ")).unwrap();
        assert_eq!(model.word(model.entries(second)[0].word), "測試");
        assert!(model.child(first, syllable("ㄘㄜˋ")).is_none());
    }

    #[test]
    fn walks_every_tone_of_a_syllable_at_once() {
        let mut builder = ModelBuilder::new();
        for (word, reading) in [("詩", "ㄕ"), ("十", "ㄕˊ"), ("是", "ㄕˋ"), ("書", "ㄕㄨ")] {
            assert!(builder.add_entry(word, reading, 0.0));
        }
        let model = Model::from_bytes(&builder.build()).unwrap();
        let words = |syllable: Syllable| -> Vec<&str> {
            let nodes = model.children(0, syllable).iter();
            nodes.map(|&node| model.word(model.entries(node)[0].word)).collect()
        };
        let any_tone = Syllable::any_tone(syllable("ㄕˋ").parts()).unwrap();
        assert_eq!(words(any_tone), ["詩", "十", "是"], "不包含 ㄕㄨ");
        assert_eq!(words(syllable("ㄕˋ")), ["是"]);
        assert!(model.children(0, syllable("ㄕˇ")).is_empty());
        assert!(model.is_valid_syllable(any_tone));
        assert!(!model.is_valid_syllable(Syllable::any_tone(syllable("ㄘㄠ").parts()).unwrap()));
    }

    #[test]
    fn knows_which_syllables_exist() {
        let model = sample();
        assert!(model.is_valid_syllable(syllable("ㄕˋ")));
        assert!(!model.is_valid_syllable(syllable("ㄘㄠˇ")));
    }

    #[test]
    fn bigram_beats_unigram_only_score() {
        let model = sample();
        let (ce, shi) = (model.word_id("測").unwrap(), model.word_id("試").unwrap());
        assert!(model.score(Some(ce), shi) > model.score(None, shi));
        assert_eq!(model.best_score(shi), model.score(Some(ce), shi));
        assert_eq!(model.best_score(ce), model.score(None, ce), "沒有 bigram 的詞");
        assert_eq!(model.score(Some(shi), ce), model.score(None, ce));
    }

    #[test]
    fn unknown_words_get_the_floor_probability() {
        let mut builder = ModelBuilder::new();
        builder.add_entry("冊", "ㄘㄜˋ", 0.0);
        let model = Model::from_bytes(&builder.build()).unwrap();
        assert_eq!(model.unigram(0), -8.0);
    }

    #[test]
    fn rejects_entries_whose_reading_does_not_match() {
        let mut builder = ModelBuilder::new();
        assert!(!builder.add_entry("測試", "ㄘㄜˋ", 0.0), "音節數與字數不同");
        assert!(!builder.add_entry("冊", "abc", 0.0), "讀音不是注音");
    }

    #[test]
    fn rejects_bytes_that_are_not_a_data_file() {
        assert!(Model::from_bytes(b"hello").is_err());
        let mut truncated = sample_bytes();
        truncated.truncate(truncated.len() - 3);
        assert!(Model::from_bytes(&truncated).is_err());
    }

    fn sample_bytes() -> Vec<u8> {
        let mut builder = ModelBuilder::new();
        builder.add_entry("冊", "ㄘㄜˋ", 0.0);
        builder.build()
    }

    #[test]
    fn for_each_entry_visits_everything() {
        let model = sample();
        let mut seen = Vec::new();
        model.for_each_entry(|word, reading, _| {
            let text: Vec<String> = reading.iter().map(|s| s.to_string()).collect();
            seen.push(format!("{word}:{}", text.join(" ")));
        });
        seen.sort();
        assert_eq!(seen, ["冊:ㄘㄜˋ", "測:ㄘㄜˋ", "測試:ㄘㄜˋ ㄕˋ", "試:ㄕˋ"]);
    }
}
