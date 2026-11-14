//! 印出準確率報告。
//!
//! ```text
//! mofoim-accuracy [資料檔] [句子檔] [--typo-rate 0.03] [--penalty -3.0] [--max-typos 1]
//! ```

use mofoim_accuracy::{measure, Report};
use mofoim_engine::{Config, Model};
use std::sync::Arc;

fn main() {
    let mut paths = Vec::new();
    let mut typo_rate = 0.03;
    let mut config = Config::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().expect("選項後面要有數值");
        match arg.as_str() {
            "--typo-rate" => typo_rate = value().parse().expect("--typo-rate 要是數字"),
            "--penalty" => config.typo_penalty = value().parse().expect("--penalty 要是數字"),
            "--max-typos" => {
                config.max_typos_per_syllable = value().parse().expect("--max-typos 要是整數")
            }
            _ => paths.push(arg),
        }
    }
    let data = paths.first().map_or("data/mofoim.dat", String::as_str);
    let sentences = paths.get(1).map_or("testdata/sentences.txt", String::as_str);

    let bytes = std::fs::read(data).unwrap_or_else(|e| panic!("讀不到資料檔 {data}：{e}"));
    let model = Arc::new(Model::from_bytes(&bytes).expect("資料檔格式不對"));
    let text = std::fs::read_to_string(sentences)
        .unwrap_or_else(|e| panic!("讀不到句子檔 {sentences}：{e}"));
    let sentences: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();

    println!(
        "容錯：每個音節最多 {} 次誤觸，每次扣 {} 分；誤觸測試的機率：每個按鍵 {}%",
        config.max_typos_per_syllable,
        config.typo_penalty,
        typo_rate * 100.0
    );
    print("完全打對", &measure(&model, &config, &sentences, 0.0, 1));
    print("加入誤觸", &measure(&model, &config, &sentences, typo_rate, 1));
}

fn print(label: &str, report: &Report) {
    println!(
        "{label}：字 {:.2}%，整句 {:.2}%（{} 句，略過 {} 句）；每鍵平均 {:.3} 毫秒，最慢 {:.2} 毫秒",
        report.character_accuracy(),
        report.sentence_accuracy(),
        report.sentences,
        report.skipped,
        report.average_ms(),
        report.slowest_ms
    );
}
