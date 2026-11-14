//! 印出準確率報告。
//!
//! ```text
//! mofoim-accuracy [資料檔] [句子檔] [--typo-rate 0.03] [--penalty -3.5] [--max-typos 2] [--beam 80]
//!                 [--toneless-penalty -0.5] [--toneless-max-typos 1] [--toneless-rate 1.0]
//!                 [--require-tones] [--show-misses 20]
//! ```
//!
//! `--toneless-rate` 是測試「不打聲調」時省略多少比例的聲調鍵；`--require-tones` 改成聲調
//! 一定要打，用來對照；`--show-misses` 列出幾個不打聲調時猜錯的句子。

use mofoim_accuracy::{drop_tones, measure_typing, Report, Rng, Typing, Typist};
use mofoim_engine::{Config, Engine, Key, Model};
use std::sync::Arc;

fn main() {
    let mut paths = Vec::new();
    let mut typo_rate = 0.03;
    let mut toneless_rate = 1.0;
    let mut misses = 0;
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
            "--beam" => config.beam = value().parse().expect("--beam 要是整數"),
            "--toneless-penalty" => {
                config.toneless_penalty =
                    Some(value().parse().expect("--toneless-penalty 要是數字"))
            }
            "--toneless-max-typos" => {
                config.max_typos_per_toneless_syllable =
                    value().parse().expect("--toneless-max-typos 要是整數")
            }
            "--toneless-rate" => {
                toneless_rate = value().parse().expect("--toneless-rate 要是數字")
            }
            "--require-tones" => config.toneless_penalty = None,
            "--show-misses" => misses = value().parse().expect("--show-misses 要是整數"),
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
        "容錯：每個音節最多 {} 次誤觸，每次扣 {} 分；誤觸測試的機率：每個按鍵 {}%；每個位置保留 {} 條路徑",
        config.max_typos_per_syllable,
        config.typo_penalty,
        typo_rate * 100.0,
        config.beam
    );
    let measure = |label: &str, typing: Typing| {
        print(label, &measure_typing(&model, &config, &sentences, Typing { seed: 1, ..typing }));
    };
    let run = |label: &str, typo_rate: f64, toneless_rate: f64| {
        measure(label, Typing { typo_rate, toneless_rate, ..Typing::default() });
    };
    run("完全打對", 0.0, 0.0);

    run("加入誤觸", typo_rate, 0.0);
    measure("符號順序顛倒", Typing { reversed: true, ..Typing::default() });
    measure("順序顛倒加誤觸", Typing { reversed: true, typo_rate, ..Typing::default() });
    if let Some(penalty) = config.toneless_penalty {
        println!(
            "不打聲調：每個沒打聲調的音節扣 {} 分，最多 {} 次誤觸；省略 {}% 的聲調鍵",
            penalty,
            config.max_typos_per_toneless_syllable,
            toneless_rate * 100.0
        );
        run("不打聲調", 0.0, toneless_rate);
        run("不打聲調加誤觸", typo_rate, toneless_rate);
        show_misses(&model, &config, &sentences, toneless_rate, misses);
    }
}

/// 印出前 `limit` 個「不打聲調、沒有誤觸」時猜錯的句子，方便看錯在哪裡。
fn show_misses(model: &Arc<Model>, config: &Config, sentences: &[&str], rate: f64, limit: usize) {
    let typist = Typist::new(model);
    let mut rng = Rng::new(1);
    let mut shown = 0;
    for sentence in sentences {
        if shown == limit {
            break;
        }
        let Some(keys) = typist.keys_for(sentence) else { continue };
        let keys = drop_tones(&keys, typist.layout(), rate, &mut rng);
        let mut engine = Engine::new(model.clone(), config.clone());
        for key in keys.chars() {
            engine.handle_key(Key::Char(key));
        }
        let guessed = engine.snapshot().text();
        if guessed != *sentence {
            println!("  原句：{sentence}\n  猜成：{guessed}");
            shown += 1;
        }
    }
}

fn print(label: &str, report: &Report) {
    println!(
        "{label}：字 {:.2}%，整句 {:.2}%（{} 句，略過 {} 句，共 {} 鍵）；每鍵平均 {:.3} 毫秒，最慢 {:.2} 毫秒",
        report.character_accuracy(),
        report.sentence_accuracy(),
        report.sentences,
        report.skipped,
        report.keys,
        report.average_ms(),
        report.slowest_ms
    );
}
