//! 引擎行為測試：手動選字與釘住、英文模式、原樣送出、標點。

mod common;

use common::{engine, type_keys};
use mofoim_engine::{CellKind, Engine, Key, Mode};

fn kinds(engine: &Engine) -> Vec<CellKind> {
    engine.snapshot().cells.iter().map(|c| c.kind).collect()
}

/// 把第 `cell` 格釘住成候選字詞 `word`。
fn pin(engine: &mut Engine, cell: usize, word: &str) -> mofoim_engine::Snapshot {
    let index = engine.candidates(cell).iter().position(|c| c == word);
    engine.pin(cell, index.unwrap_or_else(|| panic!("候選字詞裡沒有「{word}」")))
}

// ---- 候選字與釘住 ----

#[test]
fn candidates_list_same_sounding_characters_most_likely_first() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    assert_eq!(engine.candidates(0)[..3], ["測", "側", "冊"]);
}

#[test]
fn candidates_offer_longer_words_before_single_characters() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    assert_eq!(engine.candidates(0)[..2], ["測試", "測"]);
    assert_eq!(engine.candidates(1)[..3], ["是", "事", "試"]);
}

#[test]
fn candidates_include_neighboring_key_readings_after_the_literal_ones() {
    let mut engine = engine();
    // hl3 照字面是「草」。把 l、3 當成按錯的 k、4 就是 ㄘㄜˋ（測、側、冊）；
    // 把 h、l 當成按錯的 g、. 就是 ㄕㄡˇ（手、首）。這些排在照字面的讀法後面。
    type_keys(&mut engine, "hl3");
    assert_eq!(engine.candidates(0), ["草", "手", "測", "首", "側", "冊"]);
}

#[test]
fn pinning_replaces_the_guess_and_reports_what_was_pinned() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    let snapshot = pin(&mut engine, 0, "冊");
    assert_eq!(snapshot.text(), "冊");
    assert_eq!(snapshot.cells[0].kind, CellKind::Pinned);
    assert_eq!(snapshot.pinned.as_deref(), Some("冊"));
}

#[test]
fn a_pinned_cell_is_not_changed_by_later_keys() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    pin(&mut engine, 0, "冊");
    // 沒釘住的話，後面接 ㄕˋ 會整個變成「測試」。
    assert_eq!(type_keys(&mut engine, "g4").text(), "冊是");
    assert_eq!(kinds(&engine), [CellKind::Pinned, CellKind::Guess]);
}

#[test]
fn unpinned_cells_still_follow_the_context_around_a_pin() {
    let mut engine = engine();
    type_keys(&mut engine, "u 1p3gj ");
    assert_eq!(engine.snapshot().text(), "一本書");
    pin(&mut engine, 2, "輸");
    assert_eq!(engine.snapshot().text(), "一本輸");
    assert_eq!(kinds(&engine), [CellKind::Guess, CellKind::Guess, CellKind::Pinned]);
}

#[test]
fn pinning_one_character_of_a_word_splits_the_word() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    assert_eq!(pin(&mut engine, 1, "事").text(), "測事");
}

#[test]
fn pinning_a_word_covers_all_its_characters() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    pin(&mut engine, 1, "事");
    assert_eq!(pin(&mut engine, 0, "測試").text(), "測試");
    assert_eq!(kinds(&engine), [CellKind::Pinned, CellKind::Pinned]);
}

#[test]
fn the_user_can_pin_the_neighboring_key_reading() {
    let mut engine = engine();
    type_keys(&mut engine, "hl3");
    assert_eq!(pin(&mut engine, 0, "冊").text(), "冊");
}

#[test]
fn deleting_into_a_pinned_cell_removes_the_pin() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    pin(&mut engine, 0, "冊");
    assert_eq!(engine.handle_key(Key::Backspace).text(), "ㄘㄜ");
    assert_eq!(type_keys(&mut engine, "4").text(), "測");
    assert_eq!(kinds(&engine), [CellKind::Guess]);
}

#[test]
fn cells_that_are_not_characters_have_no_candidates() {
    let mut engine = engine();
    type_keys(&mut engine, "ji3h");
    engine.handle_key(Key::Shift(','));
    assert_eq!(engine.snapshot().text(), "我ㄘ，");
    assert!(engine.candidates(1).is_empty(), "注音符號");
    assert!(engine.candidates(2).is_empty(), "標點");
    assert!(engine.candidates(9).is_empty(), "不存在的位置");
    let snapshot = engine.pin(1, 0);
    assert!(!snapshot.handled);
    assert_eq!(snapshot.pinned, None);
}

// ---- 英文模式 ----

#[test]
fn toggling_switches_between_chinese_and_english_mode() {
    let mut engine = engine();
    assert_eq!(engine.snapshot().mode, Mode::Chinese);
    assert_eq!(engine.handle_key(Key::ToggleMode).mode, Mode::English);
    assert_eq!(engine.handle_key(Key::ToggleMode).mode, Mode::Chinese);
}

#[test]
fn english_letters_join_the_composition_as_typed() {
    let mut engine = engine();
    type_keys(&mut engine, "ji3g4");
    engine.handle_key(Key::ToggleMode);
    type_keys(&mut engine, "ok");
    engine.handle_key(Key::Shift('a'));
    type_keys(&mut engine, " 1");
    assert_eq!(engine.snapshot().text(), "我是okA 1");
    engine.handle_key(Key::ToggleMode);
    assert_eq!(type_keys(&mut engine, "g4").text(), "我是okA 1是");
    assert_eq!(engine.handle_key(Key::Enter).committed, "我是okA 1是");
}

#[test]
fn english_letters_are_shown_as_literal_cells() {
    let mut engine = engine();
    type_keys(&mut engine, "ji3");
    engine.handle_key(Key::ToggleMode);
    type_keys(&mut engine, "hk4");
    assert_eq!(engine.snapshot().text(), "我hk4");
    assert_eq!(kinds(&engine)[1..], [CellKind::Literal; 3]);
}

#[test]
fn english_mode_leaves_keys_alone_when_nothing_is_being_composed() {
    let mut engine = engine();
    engine.handle_key(Key::ToggleMode);
    for key in [Key::Char('h'), Key::Shift('h'), Key::Char(' '), Key::Char('1')] {
        let snapshot = engine.handle_key(key);
        assert!(!snapshot.handled, "{key:?}");
        assert!(snapshot.cells.is_empty());
    }
}

#[test]
fn shift_with_a_letter_types_a_capital_in_chinese_mode() {
    let mut engine = engine();
    engine.handle_key(Key::Shift('a'));
    assert_eq!(type_keys(&mut engine, "ji3").text(), "A我");
}

// ---- 標點與空格 ----

#[test]
fn shift_with_punctuation_keys_types_full_width_marks() {
    let mut engine = engine();
    type_keys(&mut engine, "ji3");
    for key in [',', '.', '/', '1', ';'] {
        engine.handle_key(Key::Shift(key));
    }
    assert_eq!(engine.snapshot().text(), "我，。？！：");
}

#[test]
fn keys_not_used_by_zhuyin_type_marks_directly() {
    let mut engine = engine();
    type_keys(&mut engine, "ji3'[g4]");
    assert_eq!(engine.snapshot().text(), "我、「是」");
}

#[test]
fn punctuation_does_not_stop_the_words_around_it_from_being_guessed() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4");
    engine.handle_key(Key::Shift(','));
    assert_eq!(type_keys(&mut engine, "hk4g4").text(), "手冊，測試");
}

#[test]
fn space_after_a_finished_syllable_is_a_space() {
    let mut engine = engine();
    assert_eq!(type_keys(&mut engine, "ji3 g4").text(), "我 是");
}

// ---- 原樣送出 ----

#[test]
fn raw_send_gives_back_the_keys_as_english() {
    let mut engine = engine();
    // 忘了切英文模式就打了 hello。
    type_keys(&mut engine, "hello");
    let snapshot = engine.handle_key(Key::ShiftEnter);
    assert_eq!(snapshot.committed, "hello");
    assert!(snapshot.cells.is_empty());
}

#[test]
fn raw_send_keeps_capitals_spaces_and_shifted_symbols() {
    let mut engine = engine();
    engine.handle_key(Key::Shift('h'));
    type_keys(&mut engine, "i, mofoim");
    engine.handle_key(Key::Shift('1'));
    assert_eq!(engine.handle_key(Key::ShiftEnter).committed, "Hi, mofoim!");
}

#[test]
fn raw_send_ignores_pins_and_guesses() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    pin(&mut engine, 0, "冊");
    assert_eq!(engine.handle_key(Key::ShiftEnter).committed, "hk4");
}
