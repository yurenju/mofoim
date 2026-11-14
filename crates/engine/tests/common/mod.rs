//! 測試共用的小型詞庫與打字輔助函式。
#![allow(dead_code)]

use mofoim_engine::{Config, Engine, Key, Model, ModelBuilder, Snapshot};
use std::sync::Arc;

/// 一份手寫的小詞庫：詞、讀音、單獨出現的機率（log10）。
const WORDS: &[(&str, &str, f32)] = &[
    ("測", "ㄘㄜˋ", -3.6),
    ("側", "ㄘㄜˋ", -4.2),
    ("冊", "ㄘㄜˋ", -4.6),
    ("草", "ㄘㄠˇ", -4.1),
    ("手", "ㄕㄡˇ", -3.3),
    ("首", "ㄕㄡˇ", -3.8),
    ("手冊", "ㄕㄡˇ ㄘㄜˋ", -4.8),
    ("測試", "ㄘㄜˋ ㄕˋ", -3.9),
    ("是", "ㄕˋ", -2.0),
    ("事", "ㄕˋ", -2.9),
    ("試", "ㄕˋ", -3.7),
    ("我", "ㄨㄛˇ", -2.2),
    ("中", "ㄓㄨㄥ", -2.6),
    ("文", "ㄨㄣˊ", -3.2),
    ("中文", "ㄓㄨㄥ ㄨㄣˊ", -3.6),
    ("打", "ㄉㄚˇ", -3.3),
    ("字", "ㄗˋ", -3.2),
    ("打字", "ㄉㄚˇ ㄗˋ", -4.3),
    ("一", "ㄧ", -2.0),
    ("本", "ㄅㄣˇ", -3.0),
    ("書", "ㄕㄨ", -3.4),
    ("輸", "ㄕㄨ", -3.5),
    ("個", "ㄍㄜˋ", -2.5),
];

const BIGRAMS: &[(&str, &str, f32)] = &[
    ("一", "本", -1.2),
    ("本", "書", -0.8),
    ("本", "手冊", -0.8),
    ("我", "是", -1.0),
];

pub fn model() -> Arc<Model> {
    let mut builder = ModelBuilder::new();
    for &(word, reading, probability) in WORDS {
        assert!(builder.add_entry(word, reading, 0.0), "{word}");
        builder.set_unigram(word, probability);
    }
    for &(first, second, probability) in BIGRAMS {
        builder.add_bigram(first, second, probability);
    }
    Arc::new(Model::from_bytes(&builder.build()).unwrap())
}

pub fn engine() -> Engine {
    Engine::new(model(), Config::default())
}

pub fn engine_with(config: Config) -> Engine {
    Engine::new(model(), config)
}

/// 依序按下 `keys` 裡的每個字元鍵，回傳最後一次的快照。
pub fn type_keys(engine: &mut Engine, keys: &str) -> Snapshot {
    let mut last = engine.snapshot();
    for ch in keys.chars() {
        last = engine.handle_key(Key::Char(ch));
    }
    last
}

/// 按下所有按鍵後編輯區的文字。
pub fn typed(keys: &str) -> String {
    type_keys(&mut engine(), keys).text()
}

/// 把編輯區畫成一行字，方便比對：釘住的段用方括號包起來，音節之間用 `·` 隔開，
/// 游標是 `|`。例如 `[手]ㄘㄜˋ·ㄕ|ˋ`。
pub fn shown(snapshot: &Snapshot) -> String {
    let pinned: String = snapshot.pinned.iter().map(|text| format!("[{text}]")).collect();
    format!("{pinned}{}|{}", snapshot.before_cursor.join("·"), snapshot.after_cursor.join("·"))
}

/// 候選字的文字。
pub fn candidates(snapshot: &Snapshot) -> Vec<&str> {
    snapshot.candidates.iter().map(|c| c.text.as_str()).collect()
}

/// 釘住文字是 `word` 的候選字。
pub fn pin(engine: &mut Engine, word: &str) -> Snapshot {
    let index = engine.snapshot().candidates.iter().position(|c| c.text == word);
    engine.pin(index.unwrap_or_else(|| panic!("候選字裡沒有「{word}」")))
}

/// 依序按下 `keys` 裡的每個按鍵，回傳最後一次的快照。
pub fn press(engine: &mut Engine, keys: &[Key]) -> Snapshot {
    let mut last = engine.snapshot();
    for &key in keys {
        last = engine.handle_key(key);
    }
    last
}

/// 關閉容錯的引擎，用來對照。
pub fn exact_engine() -> Engine {
    engine_with(Config { max_typos_per_syllable: 0, ..Config::default() })
}
