//! 準確率測試：用真實的詞庫、語言模型與一批固定的句子，確認引擎猜句子的能力沒有倒退。
//!
//! 需要先建置資料檔（見 README），所以預設不會執行。執行方式：
//!
//! ```text
//! cargo test --release -p mofoim-accuracy -- --ignored
//! ```

use mofoim_accuracy::{measure, measure_typing, reverse_within_syllables, Report, Typing, Typist};
use mofoim_engine::{Config, Engine, Key, Model};
use std::path::Path;
use std::sync::Arc;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_model() -> Arc<Model> {
    let data = root().join("data/mofoim.dat");
    let bytes = std::fs::read(&data)
        .unwrap_or_else(|e| panic!("讀不到 {}，請先依 README 建置資料檔：{e}", data.display()));
    Arc::new(Model::from_bytes(&bytes).unwrap())
}

fn load_sentences() -> String {
    std::fs::read_to_string(root().join("testdata/sentences.txt")).unwrap()
}

fn run(config: &Config, typo_rate: f64) -> Report {
    let text = load_sentences();
    let sentences: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    measure(&load_model(), config, &sentences, typo_rate, 1)
}

fn run_typing(config: &Config, typing: Typing) -> Report {
    let text = load_sentences();
    let sentences: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    measure_typing(&load_model(), config, &sentences, Typing { seed: 1, ..typing })
}

/// 建立這些測試時量到的數字（2,000 句，誤觸機率為每個按鍵 3%）：
///
/// | 情況                             | 字     | 整句   |
/// |----------------------------------|--------|--------|
/// | 關閉容錯、完全打對               | 96.85% | 82.80% |
/// | 關閉容錯、加入誤觸               | 79.81% | 44.35% |
/// | 開啟容錯、完全打對               | 96.85% | 82.70% |
/// | 開啟容錯、加入誤觸               | 95.04% | 75.45% |
/// | 聲調必打、開啟容錯、完全打對     | 96.87% | 82.75% |
/// | 聲調必打、開啟容錯、加入誤觸     | 95.27% | 76.25% |
/// | 完全不打聲調                     | 90.84% | 64.10% |
/// | 完全不打聲調、加入誤觸           | 87.20% | 55.20% |
/// | 一半的聲調不打                   | 94.11% | 73.60% |
/// | 一半的聲調不打、加入誤觸         | 91.44% | 65.50% |
///
/// 「聲調必打」是 ADR 0005 之前的行為，留著當比較基準。
/// 下面的門檻比量到的數字略低一點，留給之後調整詞庫或參數的空間，但擋得住明顯的倒退。
const TYPO_RATE: f64 = 0.03;

fn exact() -> Config {
    Config { max_typos_per_syllable: 0, ..Config::default() }
}

/// 聲調一定要打的設定。
fn toned() -> Config {
    Config { toneless_penalty: None, ..Config::default() }
}

/// 關閉容錯、完全打對時的準確率，是判斷「容錯有沒有把打對的字改錯」的比較基準。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn exact_typing_without_tolerance_stays_at_the_baseline() {
    let report = run(&exact(), 0.0);
    assert_eq!(report.skipped, 0, "測試句子裡不應該有詞庫沒收的字");
    assert!(report.character_accuracy() >= 96.5, "字的準確率：{:.2}%", report.character_accuracy());
    assert!(report.sentence_accuracy() >= 82.0, "整句的準確率：{:.2}%", report.sentence_accuracy());
}

/// 容錯的代價不能是把本來打對的字改錯。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn tolerance_does_not_hurt_exact_typing() {
    let baseline = run(&exact(), 0.0);
    let tolerant = run(&Config::default(), 0.0);
    let lost_characters = baseline.character_accuracy() - tolerant.character_accuracy();
    let lost_sentences = baseline.sentence_accuracy() - tolerant.sentence_accuracy();
    assert!(lost_characters <= 0.1, "字的準確率下降了 {lost_characters:.2} 個百分點");
    assert!(lost_sentences <= 0.5, "整句的準確率下降了 {lost_sentences:.2} 個百分點");
}

/// 有誤觸時，容錯要救得回大部分的字。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn tolerance_recovers_most_typos() {
    let without = run(&exact(), TYPO_RATE);
    let with = run(&Config::default(), TYPO_RATE);
    assert!(with.character_accuracy() >= 94.5, "字的準確率：{:.2}%", with.character_accuracy());
    assert!(with.sentence_accuracy() >= 75.0, "整句的準確率：{:.2}%", with.sentence_accuracy());
    assert!(with.character_accuracy() > without.character_accuracy() + 15.0);
}

/// 允許不打聲調的代價，不能是讓照常打聲調的人明顯變差。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn optional_tones_do_not_hurt_typing_with_tones() {
    for (typo_rate, characters, sentences) in [(0.0, 0.1, 0.5), (TYPO_RATE, 0.5, 1.5)] {
        let required = run(&toned(), typo_rate);
        let optional = run(&Config::default(), typo_rate);
        let lost_characters = required.character_accuracy() - optional.character_accuracy();
        let lost_sentences = required.sentence_accuracy() - optional.sentence_accuracy();
        assert!(lost_characters <= characters, "字的準確率下降了 {lost_characters:.2} 個百分點");
        assert!(lost_sentences <= sentences, "整句的準確率下降了 {lost_sentences:.2} 個百分點");
    }
}

/// 完全不打聲調時，大部分的字仍然要猜得出來；有誤觸時也一樣。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn sentences_typed_without_tones_are_mostly_guessed() {
    let toneless = Typing { toneless_rate: 1.0, ..Typing::default() };
    let clean = run_typing(&Config::default(), toneless);
    assert!(clean.character_accuracy() >= 90.0, "字的準確率：{:.2}%", clean.character_accuracy());
    assert!(clean.sentence_accuracy() >= 62.5, "整句的準確率：{:.2}%", clean.sentence_accuracy());
    let noisy = run_typing(&Config::default(), Typing { typo_rate: TYPO_RATE, ..toneless });
    assert!(noisy.character_accuracy() >= 86.5, "字的準確率：{:.2}%", noisy.character_accuracy());
    assert!(noisy.sentence_accuracy() >= 54.0, "整句的準確率：{:.2}%", noisy.sentence_accuracy());
}

/// 只省略一部分聲調時，準確率要介於全打與全不打之間，省得越少越準。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn typing_some_of_the_tones_helps() {
    let half = run_typing(&Config::default(), Typing { toneless_rate: 0.5, ..Typing::default() });
    assert!(half.character_accuracy() >= 93.5, "字的準確率：{:.2}%", half.character_accuracy());
    assert!(half.sentence_accuracy() >= 72.0, "整句的準確率：{:.2}%", half.sentence_accuracy());
}

/// 每次按鍵的運算要夠快，使用者才不會覺得卡。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn every_key_is_handled_within_the_time_budget() {
    let report = run(&Config::default(), TYPO_RATE);
    assert!(report.average_ms() < 1.0, "每鍵平均 {:.3} 毫秒", report.average_ms());
    assert!(report.slowest_ms < 10.0, "最慢的一次按鍵花了 {:.2} 毫秒", report.slowest_ms);
}

/// 一口氣打很長的內容、編輯區一直停在長度上限時，每次按鍵仍然要夠快，
/// 而且自動送出的文字加上編輯區剩下的文字，要和分段打出來的一樣長。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn a_full_composition_stays_fast() {
    let model = load_model();
    let text: String = load_sentences().lines().take(40).collect();
    let keys = Typist::new(&model).keys_for(&text).unwrap();
    let mut engine = Engine::new(model, Config::default());
    let mut sent = String::new();
    let mut slowest = 0.0f64;
    for key in keys.chars() {
        let started = std::time::Instant::now();
        let snapshot = engine.handle_key(Key::Char(key));
        slowest = slowest.max(started.elapsed().as_secs_f64() * 1000.0);
        assert!(snapshot.text().chars().count() <= 50);
        sent.push_str(&snapshot.committed);
    }
    sent.push_str(&engine.snapshot().text());
    assert_eq!(sent.chars().count(), text.chars().count());
    assert!(slowest < 10.0, "最慢的一次按鍵花了 {slowest:.2} 毫秒");
    println!("編輯區滿載時最慢的一次按鍵：{slowest:.2} 毫秒");
}

fn typed(model: &Arc<Model>, config: &Config, keys: &str) -> String {
    let mut engine = Engine::new(model.clone(), config.clone());
    for key in keys.chars() {
        engine.handle_key(Key::Char(key));
    }
    engine.snapshot().text()
}

/// 聲調一定要打時，同一個字裡的符號不管按的順序：整批句子把順序倒過來打，
/// 猜出來的結果要一模一樣。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn symbol_order_within_a_syllable_does_not_change_the_result_when_tones_are_required() {
    let model = load_model();
    let typist = Typist::new(&model);
    let text = load_sentences();
    let mut reordered = 0;
    for sentence in text.lines().filter(|l| !l.is_empty()) {
        let keys = typist.keys_for(sentence).unwrap();
        let reversed = reverse_within_syllables(&keys, typist.layout());
        reordered += (reversed != keys) as usize;
        assert_eq!(typed(&model, &toned(), &reversed), typed(&model, &toned(), &keys), "{sentence}");
    }
    assert!(reordered > 1900, "測試要真的有換到順序，實際只有 {reordered} 句");
}

/// 聲調可以不打時，順序顛倒的按鍵多了「其實是兩個沒打聲調的字」這種解讀，
/// 個別句子的結果可能不同，但整體不能比照順序打的時候差。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn symbol_order_within_a_syllable_does_not_cost_accuracy() {
    for typo_rate in [0.0, TYPO_RATE] {
        let in_order = run(&Config::default(), typo_rate);
        let reversed = Typing { reversed: true, typo_rate, ..Typing::default() };
        let reversed = run_typing(&Config::default(), reversed);
        let lost_characters = in_order.character_accuracy() - reversed.character_accuracy();
        let lost_sentences = in_order.sentence_accuracy() - reversed.sentence_accuracy();
        assert!(lost_characters <= 0.2, "字的準確率下降了 {lost_characters:.2} 個百分點");
        assert!(lost_sentences <= 0.5, "整句的準確率下降了 {lost_sentences:.2} 個百分點");
    }
}
