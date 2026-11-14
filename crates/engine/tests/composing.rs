//! 引擎行為測試：照字面打字時，編輯區與送出的行為。

mod common;

use common::{engine, engine_with, shown, toned_engine, type_keys, typed};
use mofoim_engine::{Config, Key};

#[test]
fn a_finished_syllable_becomes_the_most_likely_character() {
    let snapshot = type_keys(&mut engine(), "hk4");
    assert_eq!(snapshot.text(), "測");
    assert!(snapshot.handled);
    assert_eq!(snapshot.committed, "");
}

#[test]
fn an_unfinished_syllable_shows_the_symbols_actually_pressed_when_tones_are_required() {
    let snapshot = type_keys(&mut toned_engine(), "hk");
    assert_eq!(snapshot.text(), "ㄘㄜ");
}

#[test]
fn the_editing_area_shows_zhuyin_and_the_characters_are_only_in_the_candidates() {
    let snapshot = type_keys(&mut engine(), "g.3hk4");
    assert_eq!(shown(&snapshot), "ㄕㄡˇ·ㄘㄜˋ|");
    assert!(snapshot.pinned.is_empty());
    assert_eq!(snapshot.candidates[0].text, "手冊");
}

#[test]
fn the_editing_area_shows_the_keys_pressed_not_the_reading_the_engine_guessed() {
    // k 按成隔壁的 l：引擎猜的是「測」（ㄘㄜˋ），但編輯區要照實顯示 ㄘㄠˋ。
    let snapshot = type_keys(&mut engine(), "hl4");
    assert_eq!(snapshot.text(), "測");
    assert_eq!(shown(&snapshot), "ㄘㄠˋ|");
}

#[test]
fn keys_are_grouped_into_syllables() {
    assert_eq!(shown(&type_keys(&mut engine(), "u 1p3gj ")), "ㄧ ·ㄅㄣˇ·ㄕㄨ |", "一聲顯示成空格");
    assert_eq!(shown(&type_keys(&mut engine(), "g.3h")), "ㄕㄡˇ·ㄘ|", "還沒打完的音節自己一組");
    let unreadable = type_keys(&mut toned_engine(), "hkk4hh");
    assert_eq!(shown(&unreadable), "ㄘㄜㄜˋ·ㄘㄘ|", "聲調一定要打時，猜不出來的以聲調為界");
    // 引擎認出 4 是按錯的 e 時，照它的切法分組，而不是看到聲調就切。
    assert_eq!(shown(&type_keys(&mut engine(), "ji3g44k4")), "ㄨㄛˇ·ㄕˋ·ˋㄜˋ|");
}

#[test]
fn keys_that_form_no_syllable_stay_visible_instead_of_vanishing() {
    let typed = |keys| type_keys(&mut toned_engine(), keys).text();
    assert_eq!(typed("hh"), "ㄘㄘ");
    assert_eq!(typed("hkk4"), "ㄘㄜㄜˋ");
    assert_eq!(typed("4hk4"), "ˋ測");
}

#[test]
fn symbols_within_a_syllable_can_be_pressed_in_any_order() {
    // 兩手幾乎同時按下時，聲母、介音、韻母的順序常常顛倒，這不算打錯。
    assert_eq!(typed("kh4"), typed("hk4"));
    assert_eq!(typed(".g3kh4"), "手冊");
    assert_eq!(typed("/j5 pj6"), "中文");
    assert_eq!(typed("j/5 "), "中");
}

#[test]
fn any_order_costs_nothing_even_without_tolerance() {
    assert_eq!(type_keys(&mut common::exact_engine(), ".g3kh4").text(), "手冊");
}

#[test]
fn the_tone_key_must_still_come_last() {
    assert_eq!(type_keys(&mut common::exact_engine(), "h4k").text(), "ㄘˋㄜ");
}

#[test]
fn space_is_the_first_tone() {
    assert_eq!(typed("5j/ "), "中");
    assert_eq!(typed("5j/ jp6"), "中文");
}

#[test]
fn later_keys_change_the_guess_for_earlier_ones() {
    let mut engine = engine();
    assert_eq!(type_keys(&mut engine, "g.3").text(), "手");
    assert_eq!(type_keys(&mut engine, "hk4").text(), "手冊", "單打 ㄘㄜˋ 會是「測」，接在「手」後面要變成「冊」");
    assert_eq!(typed("hk4g4"), "測試");
}

#[test]
fn the_previous_word_influences_the_next_one() {
    assert_eq!(typed("gj "), "書");
    assert_eq!(typed("u 1p3gj "), "一本書");
    assert_eq!(typed("u 1p3g.3hk4"), "一本手冊");
}

#[test]
fn enter_sends_the_whole_composition() {
    let mut engine = engine();
    type_keys(&mut engine, "ji3g4");
    let snapshot = engine.handle_key(Key::Enter);
    assert_eq!(snapshot.committed, "我是");
    assert!(snapshot.is_empty());
    assert!(snapshot.handled);
}

#[test]
fn enter_sends_unfinished_symbols_as_shown() {
    let mut engine = toned_engine();
    type_keys(&mut engine, "ji3h");
    assert_eq!(engine.handle_key(Key::Enter).committed, "我ㄘ");
}

#[test]
fn backspace_removes_only_the_last_key() {
    let mut engine = toned_engine();
    type_keys(&mut engine, "g.3hk4");
    assert_eq!(engine.handle_key(Key::Backspace).text(), "手ㄘㄜ");
    assert_eq!(engine.handle_key(Key::Backspace).text(), "手ㄘ");
    assert_eq!(type_keys(&mut engine, "l3").text(), "手草");
}

#[test]
fn escape_clears_the_composition_without_sending() {
    let mut engine = engine();
    type_keys(&mut engine, "ji3");
    let snapshot = engine.handle_key(Key::Escape);
    assert!(snapshot.is_empty());
    assert_eq!(snapshot.committed, "");
}

#[test]
fn editing_keys_pass_through_when_nothing_is_being_composed() {
    let mut engine = engine();
    let keys = [
        Key::Enter,
        Key::ShiftEnter,
        Key::Backspace,
        Key::Escape,
        Key::Char(' '),
        Key::Left,
        Key::Right,
        Key::SyllableLeft,
        Key::SyllableRight,
        Key::End,
        Key::Down,
        Key::Up,
        Key::PageDown,
        Key::PageUp,
    ];
    for key in keys {
        let snapshot = engine.handle_key(key);
        assert!(!snapshot.handled, "{key:?}");
        assert_eq!(snapshot.highlighted, None, "{key:?}");
    }
}

#[test]
fn the_front_is_sent_automatically_when_the_composition_gets_too_long() {
    let mut engine = engine_with(Config { max_cells: 4, ..Config::default() });
    let mut sent = String::new();
    let mut last = engine.snapshot();
    for ch in "g.3hk4g.3hk4ji3g4".chars() {
        last = engine.handle_key(Key::Char(ch));
        assert!(last.text().chars().count() <= 4, "編輯區不能超過上限");
        sent.push_str(&last.committed);
    }
    assert!(sent.starts_with("手冊"), "最前面的部分要先送出，實際送出：{sent}");
    assert_eq!(sent + &last.text(), "手冊手冊我是", "自動送出不能弄丟或改動任何字");
}

#[test]
fn the_same_keys_always_give_the_same_result() {
    assert_eq!(typed("u 1p3g.3hk4"), typed("u 1p3g.3hk4"));
}
