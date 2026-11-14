//! 引擎行為測試：候選字與釘住、英文模式、原樣送出、標點。

mod common;

use common::{candidates, engine, pin, shown, type_keys};
use mofoim_engine::{Key, Mode};

// ---- 候選字與釘住 ----

#[test]
fn candidates_list_same_sounding_characters_most_likely_first() {
    let snapshot = type_keys(&mut engine(), "hk4");
    assert_eq!(candidates(&snapshot)[..3], ["測", "側", "冊"]);
}

#[test]
fn the_first_candidate_is_the_whole_sentence_and_the_rest_are_shorter_prefixes() {
    let snapshot = type_keys(&mut engine(), "hk4g4");
    assert_eq!(candidates(&snapshot)[..4], ["測試", "測", "側", "冊"]);
    let snapshot = type_keys(&mut engine(), "u 1p3g.3hk4");
    assert_eq!(candidates(&snapshot)[..2], ["一本手冊", "一"]);
}

#[test]
fn candidates_include_neighboring_key_readings_after_the_literal_ones() {
    // hl3 照字面是「草」。把 l、3 當成按錯的 k、4 就是 ㄘㄜˋ（測、側、冊）；
    // 把 h、l 當成按錯的 g、. 就是 ㄕㄡˇ（手、首）。這些排在照字面的讀法後面。
    let snapshot = type_keys(&mut engine(), "hl3");
    assert_eq!(candidates(&snapshot), ["草", "手", "測", "首", "側", "冊"]);
}

#[test]
fn nothing_typed_means_no_candidates() {
    assert!(engine().snapshot().candidates.is_empty());
}

#[test]
fn pinning_turns_the_keys_into_the_chosen_word_and_reports_it() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    let snapshot = pin(&mut engine, "冊");
    assert_eq!(shown(&snapshot), "[冊]|");
    assert_eq!(snapshot.text(), "冊");
    assert_eq!(snapshot.newly_pinned.as_deref(), Some("冊"));
    assert!(snapshot.candidates.is_empty(), "沒有還沒釘住的按鍵，就沒有候選字");
}

#[test]
fn after_pinning_a_prefix_the_remaining_keys_keep_being_guessed() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4g4");
    let snapshot = pin(&mut engine, "手");
    assert_eq!(shown(&snapshot), "[手]ㄘㄜˋ·ㄕˋ|");
    assert_eq!(candidates(&snapshot)[..4], ["測試", "測", "側", "冊"]);
    assert_eq!(snapshot.text(), "手測試");
}

#[test]
fn a_pinned_segment_is_not_changed_by_later_keys() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    pin(&mut engine, "冊");
    // 沒釘住的話，後面接 ㄕˋ 會整個變成「測試」。
    let snapshot = type_keys(&mut engine, "g4");
    assert_eq!(shown(&snapshot), "[冊]ㄕˋ|");
    assert_eq!(snapshot.text(), "冊是");
}

#[test]
fn keys_after_a_pinned_segment_are_guessed_with_it_as_context() {
    let mut engine = engine();
    assert_eq!(type_keys(&mut engine, "g.3hl3").text(), "手草");
    engine.handle_key(Key::Escape);
    // 同樣的按鍵接在釘住的「本」後面，「手冊」比「手草」合理。
    type_keys(&mut engine, "1p3");
    pin(&mut engine, "本");
    assert_eq!(type_keys(&mut engine, "g.3hl3").text(), "本手冊");
}

#[test]
fn the_user_can_pin_the_neighboring_key_reading() {
    let mut engine = engine();
    type_keys(&mut engine, "hl3");
    assert_eq!(pin(&mut engine, "冊").text(), "冊");
}

#[test]
fn pinning_the_whole_sentence_guess_pins_everything_before_the_cursor() {
    // 聲調一定要打時，最後的 ㄘ 還猜不出來，也跟著一起釘住。
    let mut engine = common::toned_engine();
    type_keys(&mut engine, "ji3h");
    assert_eq!(shown(&engine.pin(0)), "[我ㄘ]|");
}

#[test]
fn leading_punctuation_can_be_pinned_on_its_own() {
    let mut engine = engine();
    engine.handle_key(Key::Shift(','));
    let snapshot = type_keys(&mut engine, "hk4");
    assert_eq!(candidates(&snapshot), ["，測", "，"]);
    assert_eq!(candidates(&pin(&mut engine, "，"))[..3], ["測", "側", "冊"]);
}

#[test]
fn pinning_a_candidate_that_does_not_exist_does_nothing() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    let snapshot = engine.pin(99);
    assert!(!snapshot.handled);
    assert_eq!(snapshot.newly_pinned, None);
    assert_eq!(shown(&snapshot), "ㄘㄜˋ|");
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
fn english_letters_are_shown_as_typed_in_the_editing_area() {
    let mut engine = engine();
    type_keys(&mut engine, "ji3");
    engine.handle_key(Key::ToggleMode);
    type_keys(&mut engine, "hk4");
    assert_eq!(engine.snapshot().text(), "我hk4");
    assert_eq!(shown(&engine.snapshot()), "ㄨㄛˇ·hk4|");
}

#[test]
fn english_mode_leaves_keys_alone_when_nothing_is_being_composed() {
    let mut engine = engine();
    engine.handle_key(Key::ToggleMode);
    for key in [Key::Char('h'), Key::Shift('h'), Key::Char(' '), Key::Char('1')] {
        let snapshot = engine.handle_key(key);
        assert!(!snapshot.handled, "{key:?}");
        assert!(snapshot.is_empty());
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
    assert!(snapshot.is_empty());
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
fn raw_send_ignores_pins_guesses_and_the_cursor() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    pin(&mut engine, "測");
    engine.handle_key(Key::Left);
    assert_eq!(engine.handle_key(Key::ShiftEnter).committed, "hk4g4");
}
