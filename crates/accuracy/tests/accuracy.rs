//! 準確率測試：用真實的詞庫、語言模型與一批固定的句子，確認引擎猜句子的能力沒有倒退。
//!
//! 需要先建置資料檔（見 README），所以預設不會執行。執行方式：
//!
//! ```text
//! cargo test --release -p mofoim-accuracy -- --ignored
//! ```

use mofoim_accuracy::{measure, Report};
use mofoim_engine::{Config, Model};
use std::path::Path;
use std::sync::Arc;

fn run(config: &Config, typo_rate: f64) -> Report {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = root.join("data/mofoim.dat");
    let bytes = std::fs::read(&data)
        .unwrap_or_else(|e| panic!("讀不到 {}，請先依 README 建置資料檔：{e}", data.display()));
    let model = Arc::new(Model::from_bytes(&bytes).unwrap());
    let text = std::fs::read_to_string(root.join("testdata/sentences.txt")).unwrap();
    let sentences: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    measure(&model, config, &sentences, typo_rate, 1)
}

/// 關閉容錯、完全打對時的準確率。這是之後判斷「容錯有沒有把打對的字改錯」的比較基準。
/// 建立這個測試時量到的數字是：字 96.87%，整句 82.85%。
#[test]
#[ignore = "需要 data/mofoim.dat"]
fn exact_typing_without_tolerance_stays_at_the_baseline() {
    let config = Config { max_typos_per_syllable: 0, ..Config::default() };
    let report = run(&config, 0.0);
    assert_eq!(report.skipped, 0, "測試句子裡不應該有詞庫沒收的字");
    assert!(report.character_accuracy() >= 96.5, "字的準確率：{:.2}%", report.character_accuracy());
    assert!(report.sentence_accuracy() >= 82.0, "整句的準確率：{:.2}%", report.sentence_accuracy());
}
