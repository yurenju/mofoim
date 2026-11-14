//! 準確率測試的工具：把句子轉成「照理應該按的鍵」，可以隨機加入誤觸，
//! 再交給引擎，量測猜出來的句子和原句有多像。

use mofoim_engine::syllable::Slot;
use mofoim_engine::{Config, Engine, Key, Layout, Model, Syllable};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// 一個詞最長幾個字（斷詞時的上限）。
const MAX_WORD_CHARS: usize = 8;

/// 由詞庫反查「這個詞怎麼打」。
pub struct Typist {
    readings: HashMap<String, (Vec<Syllable>, f32)>,
    layout: Layout,
}

impl Typist {
    pub fn new(model: &Model) -> Typist {
        let ids: HashMap<&str, u32> =
            (0..model.word_count() as u32).map(|id| (model.word(id), id)).collect();
        // 每個詞選一個讀音：優先選不是罕用的，其次選編號小的，讓結果固定。
        let mut chosen: HashMap<String, (Vec<Syllable>, f32)> = HashMap::new();
        model.for_each_entry(|word, reading, penalty| {
            let better = match chosen.get(word) {
                Some((old, old_penalty)) => {
                    penalty > *old_penalty || (penalty == *old_penalty && reading < old.as_slice())
                }
                None => true,
            };
            if better {
                chosen.insert(word.to_string(), (reading.to_vec(), penalty));
            }
        });
        let readings = chosen
            .into_iter()
            .map(|(word, (reading, _))| {
                let probability = model.unigram(ids[word.as_str()]);
                (word, (reading, probability))
            })
            .collect();
        Typist { readings, layout: Layout::dachen() }
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// 打出 `text` 要依序按的鍵。先把句子斷成詞庫裡最常見的詞，再用每個詞的讀音。
    /// 句子裡有詞庫沒收的字時回傳 `None`。
    pub fn keys_for(&self, text: &str) -> Option<String> {
        let chars: Vec<char> = text.chars().collect();
        // best[i]：前 i 個字的最佳斷法的分數，以及最後一個詞從哪裡開始。
        let mut best: Vec<Option<(f32, usize)>> = vec![None; chars.len() + 1];
        best[0] = Some((0.0, 0));
        for end in 1..=chars.len() {
            for start in end.saturating_sub(MAX_WORD_CHARS)..end {
                let Some((score, _)) = best[start] else { continue };
                let word: String = chars[start..end].iter().collect();
                let Some((_, probability)) = self.readings.get(&word) else { continue };
                let total = score + probability;
                if best[end].is_none_or(|(old, _)| total > old) {
                    best[end] = Some((total, start));
                }
            }
        }
        let mut words = Vec::new();
        let mut end = chars.len();
        while end > 0 {
            let (_, start) = best[end]?;
            words.push(chars[start..end].iter().collect::<String>());
            end = start;
        }
        let mut keys = String::new();
        for word in words.iter().rev() {
            for syllable in &self.readings[word].0 {
                for symbol in syllable.symbols() {
                    keys.push(self.layout.key_for(symbol)?);
                }
            }
        }
        Some(keys)
    }
}

/// 固定種子的亂數產生器，讓每次測試加入的誤觸都一樣。
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// 0 以上、未滿 1 的亂數。
    fn fraction(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// 每個按鍵以 `rate` 的機率換成它的某個相鄰鍵。空白鍵沒有相鄰鍵，不會被換掉。
pub fn add_typos(keys: &str, layout: &Layout, rate: f64, rng: &mut Rng) -> String {
    keys.chars()
        .map(|key| {
            let neighbors = layout.neighbors(key);
            if neighbors.is_empty() || rng.fraction() >= rate {
                key
            } else {
                neighbors[(rng.next() % neighbors.len() as u64) as usize]
            }
        })
        .collect()
}

/// 每個聲調鍵（包含代表一聲的空白鍵）以 `rate` 的機率不按。
pub fn drop_tones(keys: &str, layout: &Layout, rate: f64, rng: &mut Rng) -> String {
    keys.chars()
        .filter(|&key| {
            let tone = layout.symbol(key).is_some_and(|s| s.slot == Slot::Tone);
            !tone || rng.fraction() >= rate
        })
        .collect()
}

/// 把每個字裡聲調鍵之前的按鍵順序整個倒過來，模擬兩手幾乎同時按下時順序顛倒。
pub fn reverse_within_syllables(keys: &str, layout: &Layout) -> String {
    let mut out = String::new();
    let mut pending: Vec<char> = Vec::new();
    for key in keys.chars() {
        if layout.symbol(key).is_some_and(|s| s.slot == Slot::Tone) {
            out.extend(pending.drain(..).rev());
            out.push(key);
        } else {
            pending.push(key);
        }
    }
    out.extend(pending);
    out
}

/// 模擬的打字方式。
#[derive(Clone, Copy, Debug, Default)]
pub struct Typing {
    /// 每個按鍵變成誤觸的機率。
    pub typo_rate: f64,
    /// 每個聲調鍵被省略的機率；1.0 代表完全不打聲調。
    pub toneless_rate: f64,
    /// 每個字裡聲調鍵之前的按鍵是否倒過來按。
    pub reversed: bool,
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Report {
    /// 參與測試的句數（詞庫沒收的字會讓句子被略過）。
    pub sentences: usize,
    pub skipped: usize,
    pub characters: usize,
    pub correct_characters: usize,
    /// 整句完全猜對的句數。
    pub exact_sentences: usize,
    pub keys: usize,
    /// 所有按鍵的運算時間總和，以及最慢的一次，單位是毫秒。
    pub total_ms: f64,
    pub slowest_ms: f64,
}

impl Report {
    /// 字的準確率（百分比）。
    pub fn character_accuracy(&self) -> f64 {
        100.0 * self.correct_characters as f64 / self.characters.max(1) as f64
    }

    /// 整句的準確率（百分比）。
    pub fn sentence_accuracy(&self) -> f64 {
        100.0 * self.exact_sentences as f64 / self.sentences.max(1) as f64
    }

    /// 平均每個按鍵的運算時間（毫秒）。
    pub fn average_ms(&self) -> f64 {
        self.total_ms / self.keys.max(1) as f64
    }
}

/// 把每個句子打進引擎，量測猜對多少。`typo_rate` 是每個按鍵變成誤觸的機率。
pub fn measure(
    model: &Arc<Model>,
    config: &Config,
    sentences: &[&str],
    typo_rate: f64,
    seed: u64,
) -> Report {
    measure_typing(model, config, sentences, Typing { typo_rate, seed, ..Typing::default() })
}

/// 和 [`measure`] 一樣，但可以指定省略多少聲調鍵。先省略聲調，再加入誤觸。
pub fn measure_typing(
    model: &Arc<Model>,
    config: &Config,
    sentences: &[&str],
    typing: Typing,
) -> Report {
    let typist = Typist::new(model);
    let mut rng = Rng::new(typing.seed);
    // 省略聲調用另一個亂數產生器，這樣不管有沒有加誤觸，被省略的都是同一批聲調鍵。
    let mut tone_rng = Rng::new(typing.seed ^ 0x9E37_79B9_7F4A_7C15);
    let mut report = Report::default();
    for sentence in sentences {
        let Some(mut keys) = typist.keys_for(sentence) else {
            report.skipped += 1;
            continue;
        };
        if typing.reversed {
            keys = reverse_within_syllables(&keys, typist.layout());
        }
        if typing.toneless_rate > 0.0 {
            keys = drop_tones(&keys, typist.layout(), typing.toneless_rate, &mut tone_rng);
        }
        let keys = add_typos(&keys, typist.layout(), typing.typo_rate, &mut rng);
        let mut engine = Engine::new(model.clone(), config.clone());
        let mut snapshot = engine.snapshot();
        for key in keys.chars() {
            let started = Instant::now();
            snapshot = engine.handle_key(Key::Char(key));
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            report.total_ms += elapsed;
            report.slowest_ms = report.slowest_ms.max(elapsed);
        }
        let guessed = snapshot.text();
        report.sentences += 1;
        report.keys += keys.chars().count();
        report.characters += sentence.chars().count();
        let expected = sentence.chars().count();
        report.correct_characters += expected.saturating_sub(edit_distance(sentence, &guessed));
        report.exact_sentences += (guessed == *sentence) as usize;
    }
    report
}

/// 把 `a` 改成 `b` 最少要增、刪、改幾個字。用它算錯字數，
/// 猜出來的句子多一個字或少一個字時，後面的字才不會全部被算成錯的。
pub fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = diagonal + (ca != *cb) as usize;
            diagonal = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_counts_changed_added_and_missing_characters() {
        assert_eq!(edit_distance("一本手冊", "一本手冊"), 0);
        assert_eq!(edit_distance("一本手冊", "一本手草"), 1);
        assert_eq!(edit_distance("一本手冊", "一手冊"), 1);
        assert_eq!(edit_distance("一本手冊", "一本ㄕㄡ手冊"), 2);
    }

    #[test]
    fn typos_only_ever_move_to_a_neighboring_key() {
        let layout = Layout::dachen();
        let keys = "g.3hk4 5j/ jp6";
        let noisy = add_typos(keys, &layout, 0.5, &mut Rng::new(7));
        assert_ne!(noisy, keys);
        for (original, typed) in keys.chars().zip(noisy.chars()) {
            assert!(original == typed || layout.neighbors(original).contains(&typed));
        }
    }

    #[test]
    fn typos_are_reproducible_and_absent_at_rate_zero() {
        let layout = Layout::dachen();
        let keys = "g.3hk4 5j/ jp6";
        assert_eq!(add_typos(keys, &layout, 0.0, &mut Rng::new(7)), keys);
        assert_eq!(
            add_typos(keys, &layout, 0.3, &mut Rng::new(7)),
            add_typos(keys, &layout, 0.3, &mut Rng::new(7))
        );
    }
}
