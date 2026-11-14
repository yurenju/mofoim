//! 建置工具：把上游資料轉成引擎用的資料檔，以及抽出準確率測試用的句子。
//!
//! ```text
//! mofoim-data-builder model <libchewing-data 的 dict/chewing_v4 目錄> <輸出的資料檔>
//! mofoim-data-builder sentences <cc0-sentences 的 zh-TW 目錄> <輸出的文字檔> <句數>
//! ```

use mofoim_engine::ModelBuilder;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::Path;

/// 罕用讀音的扣分（log10）：打這個讀音時，仍然優先猜常用讀音的字詞。
const RARE_READING_PENALTY: f32 = -2.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["model", source, output] => build_model(Path::new(source), Path::new(output)),
        ["sentences", source, output, count] => match count.parse() {
            Ok(count) => sample_sentences(Path::new(source), Path::new(output), count),
            Err(_) => Err(format!("句數不是數字：{count}")),
        },
        _ => Err("用法：\n  mofoim-data-builder model <chewing_v4 目錄> <輸出檔>\n  \
                  mofoim-data-builder sentences <zh-TW 目錄> <輸出檔> <句數>"
            .to_string()),
    };
    if let Err(message) = result {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn read(path: &Path) -> Result<String, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("讀不到 {}：{e}", path.display()))?;
    if text.starts_with("version https://git-lfs") {
        return Err(format!("{} 只是 Git LFS 的指標檔，請先執行 git lfs pull", path.display()));
    }
    Ok(text)
}

/// 讀 `詞,讀音` 格式的檔案，`#` 開頭的是註解。
fn csv_pairs(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once(','))
        .map(|(word, reading)| (word.trim(), reading.trim()))
}

fn is_bopomofo(ch: char) -> bool {
    ('\u{3100}'..='\u{312F}').contains(&ch) || "ˊˇˋ˙".contains(ch)
}

/// ARPA 語言模型檔裡某一段（例如 `\1-grams:`）的每一行：機率與後面的詞。
fn arpa_section<'a>(text: &'a str, header: &str) -> impl Iterator<Item = (f32, Vec<&'a str>)> {
    let header = header.to_string();
    text.lines()
        .skip_while(move |line| line.trim() != header)
        .skip(1)
        .take_while(|line| !line.starts_with('\\'))
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let probability = fields.next()?.parse().ok()?;
            Some((probability, fields.collect()))
        })
}

fn build_model(source: &Path, output: &Path) -> Result<(), String> {
    let dictionary = read(&source.join("tsi_dict.csv"))?;
    let rare = read(&source.join("rare_dict.csv"))?;
    let language_model = read(&source.join("static_lm.arpa"))?;
    let fallback_unigrams = read(&source.join("tsi_unigram.arpa"))?;

    let rare: HashSet<(&str, &str)> = csv_pairs(&rare).collect();
    let mut builder = ModelBuilder::new();
    let (mut added, mut skipped) = (0, 0);
    for (word, reading) in csv_pairs(&dictionary) {
        let penalty = if rare.contains(&(word, reading)) { RARE_READING_PENALTY } else { 0.0 };
        // 注音符號本身也被上游收成「詞」，但它們不是使用者要打的字。
        if !word.chars().all(is_bopomofo) && builder.add_entry(word, reading, penalty) {
            added += 1;
        } else {
            skipped += 1;
        }
    }

    // 以 bigram 模型自己的單一詞機率為主，它沒有的詞才用另一份詞頻補上。
    for text in [&language_model, &fallback_unigrams] {
        for (probability, words) in arpa_section(text, "\\1-grams:") {
            if let [word] = words[..] {
                builder.set_unigram(word, probability);
            }
        }
    }
    let mut bigrams = 0;
    for (probability, words) in arpa_section(&language_model, "\\2-grams:") {
        if let [first, second] = words[..] {
            builder.add_bigram(first, second, probability);
            bigrams += 1;
        }
    }

    let bytes = builder.build();
    fs::write(output, &bytes).map_err(|e| format!("寫不出 {}：{e}", output.display()))?;
    println!("詞庫：{added} 筆（略過 {skipped} 筆無法使用的）");
    println!("bigram：{bigrams} 筆");
    println!("資料檔：{}（{:.1} MB）", output.display(), bytes.len() as f64 / 1e6);
    Ok(())
}

fn is_han(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
}

/// 從語料抽出固定的一批句子片段。相同的輸入一定抽出相同的結果。
fn sample_sentences(source: &Path, output: &Path, count: usize) -> Result<(), String> {
    let mut files: Vec<_> = fs::read_dir(source)
        .map_err(|e| format!("讀不到 {}：{e}", source.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    files.sort();

    // 以標點等非漢字的字元切開，只留全是漢字、長度適中的片段。
    let mut clauses = BTreeSet::new();
    for file in &files {
        for clause in read(file)?.split(|ch| !is_han(ch)) {
            if (4..=30).contains(&clause.chars().count()) {
                clauses.insert(clause.to_string());
            }
        }
    }
    let mut clauses: Vec<String> = clauses.into_iter().collect();

    // 固定種子的洗牌，不依賴外部的亂數函式庫。
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    for i in (1..clauses.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        clauses.swap(i, (state % (i as u64 + 1)) as usize);
    }
    clauses.truncate(count);

    let text = clauses.join("\n") + "\n";
    fs::write(output, text).map_err(|e| format!("寫不出 {}：{e}", output.display()))?;
    println!("抽出 {} 句，寫到 {}", clauses.len(), output.display());
    Ok(())
}
