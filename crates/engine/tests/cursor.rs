//! 引擎行為測試：游標、撤銷釘住、選字狀態。

mod common;

use common::{candidates, engine, pin, press, shown, type_keys};
use mofoim_engine::{Config, Engine, Key, Model, ModelBuilder};
use std::sync::Arc;

// ---- 游標 ----

#[test]
fn left_and_right_move_the_cursor_one_key_at_a_time() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4");
    assert_eq!(shown(&engine.handle_key(Key::Left)), "ㄕㄡˇ·ㄘㄜ|ˋ");
    assert_eq!(shown(&engine.handle_key(Key::Left)), "ㄕㄡˇ·ㄘ|ㄜˋ");
    assert_eq!(shown(&engine.handle_key(Key::Right)), "ㄕㄡˇ·ㄘㄜ|ˋ");
}

#[test]
fn the_cursor_stops_at_both_ends() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    assert_eq!(shown(&engine.handle_key(Key::Right)), "ㄘㄜˋ|");
    let snapshot = press(&mut engine, &[Key::Left; 5]);
    assert_eq!(shown(&snapshot), "|ㄘㄜˋ");
    assert!(snapshot.handled);
}

#[test]
fn ctrl_arrows_move_the_cursor_one_syllable_at_a_time() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4g4");
    assert_eq!(shown(&engine.handle_key(Key::SyllableLeft)), "ㄕㄡˇ·ㄘㄜˋ|ㄕˋ");
    assert_eq!(shown(&engine.handle_key(Key::SyllableLeft)), "ㄕㄡˇ|ㄘㄜˋ·ㄕˋ");
    assert_eq!(shown(&engine.handle_key(Key::SyllableRight)), "ㄕㄡˇ·ㄘㄜˋ|ㄕˋ");
    // 游標在音節中間時，先移到這個音節的邊界。
    engine.handle_key(Key::Left);
    assert_eq!(shown(&engine.handle_key(Key::SyllableLeft)), "ㄕㄡˇ|ㄘㄜˋ·ㄕˋ");
    engine.handle_key(Key::Right);
    assert_eq!(shown(&engine.handle_key(Key::SyllableRight)), "ㄕㄡˇ·ㄘㄜˋ|ㄕˋ");
}

#[test]
fn end_moves_the_cursor_back_to_the_end() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4");
    press(&mut engine, &[Key::Left; 4]);
    assert_eq!(shown(&engine.handle_key(Key::End)), "ㄕㄡˇ·ㄘㄜˋ|");
}

#[test]
fn candidates_only_look_at_the_keys_before_the_cursor() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    let snapshot = press(&mut engine, &[Key::Left, Key::Left]);
    assert_eq!(shown(&snapshot), "ㄘㄜˋ|ㄕˋ");
    assert_eq!(candidates(&snapshot)[..3], ["測", "側", "冊"]);
    assert_eq!(candidates(&engine.handle_key(Key::End))[0], "測試");
}

#[test]
fn typing_inserts_at_the_cursor() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3h4");
    engine.handle_key(Key::Left);
    assert_eq!(shown(&engine.handle_key(Key::Char('k'))), "ㄕㄡˇ·ㄘㄜ|ˋ");
    let snapshot = engine.handle_key(Key::End);
    assert_eq!(shown(&snapshot), "ㄕㄡˇ·ㄘㄜˋ|");
    assert_eq!(snapshot.text(), "手冊");
}

#[test]
fn backspace_deletes_the_key_before_the_cursor() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4");
    press(&mut engine, &[Key::Left, Key::Left]);
    assert_eq!(shown(&engine.handle_key(Key::Backspace)), "ㄕㄡˇ|ㄜˋ");
}

#[test]
fn a_wrong_key_can_be_fixed_in_place() {
    let mut engine = engine();
    // 想打「手冊」，k 按成隔壁的 l。引擎猜對了，但那個字是容錯改過的。
    let snapshot = type_keys(&mut engine, "g.3hl4");
    assert_eq!(snapshot.candidates[0].text, "手冊");
    assert_eq!(snapshot.candidates[0].corrected, [1]);
    // 移到 ㄠ 後面、刪掉它、補上 ㄜ，再回到最後。
    press(&mut engine, &[Key::Left, Key::Backspace, Key::Char('k')]);
    let snapshot = engine.handle_key(Key::End);
    assert_eq!(shown(&snapshot), "ㄕㄡˇ·ㄘㄜˋ|");
    assert_eq!(snapshot.candidates[0].text, "手冊");
    assert!(snapshot.candidates[0].corrected.is_empty());
}

#[test]
fn enter_pins_the_guess_before_the_cursor_when_keys_remain_after_it() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4");
    press(&mut engine, &[Key::Left; 3]);
    let snapshot = engine.handle_key(Key::Enter);
    assert_eq!(snapshot.committed, "", "還有剩下的按鍵，不送出");
    assert_eq!(shown(&snapshot), "[手]ㄘㄜˋ|");
    assert_eq!(snapshot.newly_pinned.as_deref(), Some("手"));
    let snapshot = engine.handle_key(Key::Enter);
    assert!(snapshot.committed.starts_with('手'), "沒有剩下的了才送出，釘住的段照原樣送出");
    assert!(snapshot.is_empty());
}

#[test]
fn a_wrong_guess_at_the_end_can_be_fixed_without_touching_the_rest() {
    // issue #2 的流程：前半句猜對、句尾猜錯，游標移到錯字前，Enter，再選句尾的字。
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    let snapshot = press(&mut engine, &[Key::SyllableLeft, Key::Enter, Key::Down]);
    assert_eq!(shown(&snapshot), "[測]ㄕˋ|");
    let picked = candidates(&snapshot)[1].clone();
    let snapshot = engine.handle_key(Key::Enter);
    assert_eq!(snapshot.committed, format!("測{picked}"));
    assert!(snapshot.is_empty());
}

#[test]
fn enter_at_the_end_sends_without_reporting_a_pinned_word() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    let snapshot = engine.handle_key(Key::Enter);
    assert_eq!(snapshot.committed, "測試");
    assert_eq!(snapshot.newly_pinned, None, "整句猜測不是使用者特地選的字詞");
}

#[test]
fn enter_with_nothing_before_the_cursor_only_moves_it_to_the_end() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4");
    pin(&mut engine, "手");
    engine.handle_key(Key::SyllableLeft);
    let snapshot = engine.handle_key(Key::Enter);
    assert_eq!(snapshot.committed, "");
    assert_eq!(shown(&snapshot), "[手]ㄘㄜˋ|");
}

#[test]
fn enter_sends_the_pinned_segments_followed_by_the_guess_for_the_rest() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    pin(&mut engine, "冊");
    assert_eq!(engine.handle_key(Key::Enter).committed, "冊是");
}

#[test]
fn pinning_everything_before_the_cursor_moves_the_cursor_to_the_end() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    press(&mut engine, &[Key::Left, Key::Left]);
    let snapshot = engine.pin(0);
    assert_eq!(shown(&snapshot), "[測]ㄕˋ|", "游標之後的按鍵接著參與猜測");
    assert_eq!(candidates(&snapshot)[..3], ["是", "事", "試"]);
}

#[test]
fn pinning_part_of_the_keys_before_the_cursor_leaves_the_cursor_in_place() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4g4");
    press(&mut engine, &[Key::SyllableLeft]);
    assert_eq!(shown(&pin(&mut engine, "手")), "[手]ㄘㄜˋ|ㄕˋ");
}

// ---- 撤銷釘住 ----

#[test]
fn backspace_at_the_edge_of_a_pinned_segment_unpins_it_without_deleting_a_key() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    assert_eq!(shown(&pin(&mut engine, "冊")), "[冊]|");
    let snapshot = engine.handle_key(Key::Backspace);
    assert_eq!(shown(&snapshot), "ㄘㄜˋ|");
    assert_eq!(snapshot.text(), "測", "退回注音後重新由引擎猜");
    assert_eq!(shown(&engine.handle_key(Key::Backspace)), "ㄘㄜ|");
}

#[test]
fn unpinning_undoes_one_choice_at_a_time_starting_from_the_last() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4g4");
    pin(&mut engine, "手");
    assert_eq!(shown(&pin(&mut engine, "測試")), "[手][測試]|");
    assert_eq!(shown(&engine.handle_key(Key::Backspace)), "[手]ㄘㄜˋ·ㄕˋ|");
    assert_eq!(shown(&engine.handle_key(Key::Backspace)), "[手]ㄘㄜˋ·ㄕ|", "不在邊界時照常刪按鍵");
}

#[test]
fn backspace_unpins_even_when_keys_remain_after_the_cursor() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4");
    pin(&mut engine, "手");
    assert_eq!(shown(&engine.handle_key(Key::SyllableLeft)), "[手]|ㄘㄜˋ");
    assert_eq!(shown(&engine.handle_key(Key::Backspace)), "ㄕㄡˇ|ㄘㄜˋ");
}

#[test]
fn moving_the_cursor_into_a_pinned_segment_unpins_it() {
    let mut engine = engine();
    type_keys(&mut engine, "g.3hk4");
    engine.pin(0);
    assert_eq!(shown(&engine.snapshot()), "[手冊]|");
    assert_eq!(shown(&engine.handle_key(Key::Left)), "ㄕㄡˇ·ㄘㄜ|ˋ");

    engine.handle_key(Key::End);
    engine.pin(0);
    assert_eq!(shown(&engine.handle_key(Key::SyllableLeft)), "ㄕㄡˇ|ㄘㄜˋ");
}

// ---- 選字狀態 ----

#[test]
fn down_enters_the_selecting_state_and_escape_leaves_it() {
    let mut engine = engine();
    assert_eq!(type_keys(&mut engine, "hk4g4").highlighted, None);
    // 要第一個候選字的話直接按 Enter 就好，所以一進來就反白第二個。
    assert_eq!(engine.handle_key(Key::Down).highlighted, Some(1));
    let snapshot = engine.handle_key(Key::Escape);
    assert_eq!(snapshot.highlighted, None);
    assert_eq!(shown(&snapshot), "ㄘㄜˋ·ㄕˋ|", "Esc 只離開選字狀態，不清空編輯區");
}

#[test]
fn entering_the_selecting_state_highlights_the_only_candidate_when_there_is_just_one() {
    let mut engine = common::toned_engine();
    // 聲調一定要打時，ㄘㄜ 還沒打完，只有整句猜測這一個候選字。
    assert_eq!(type_keys(&mut engine, "hk").candidates.len(), 1);
    assert_eq!(engine.handle_key(Key::Down).highlighted, Some(0));
}

#[test]
fn up_and_down_move_the_highlight_within_the_candidates() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    assert_eq!(press(&mut engine, &[Key::Down, Key::Down]).highlighted, Some(2));
    let last = engine.snapshot().candidates.len() - 1;
    assert_eq!(press(&mut engine, &[Key::Down; 9]).highlighted, Some(last), "停在最後一個");
    assert_eq!(press(&mut engine, &[Key::Up; 9]).highlighted, Some(0), "停在第一個");
}

#[test]
fn up_does_nothing_in_the_typing_state() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    let snapshot = engine.handle_key(Key::Up);
    assert!(snapshot.handled);
    assert_eq!(snapshot.highlighted, None);
}

#[test]
fn digits_pick_candidates_only_in_the_selecting_state() {
    let mut engine = engine();
    // 打字狀態下 4 是聲調。
    assert_eq!(candidates(&type_keys(&mut engine, "hk4g4"))[..4], ["測試", "測", "側", "冊"]);
    engine.handle_key(Key::Down);
    let snapshot = engine.handle_key(Key::Char('4'));
    assert_eq!(shown(&snapshot), "[冊]ㄕˋ|");
    assert_eq!(snapshot.newly_pinned.as_deref(), Some("冊"));
}

#[test]
fn after_picking_the_rest_can_be_picked_right_away() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    let snapshot = press(&mut engine, &[Key::Down, Key::Char('2')]);
    assert_eq!(shown(&snapshot), "[測]ㄕˋ|");
    assert_eq!(snapshot.highlighted, Some(0), "還有剩下的按鍵，留在選字狀態");
    assert_eq!(candidates(&snapshot)[..3], ["是", "事", "試"]);
    let snapshot = engine.handle_key(Key::Char('3'));
    assert_eq!(shown(&snapshot), "[測][試]|");
    assert_eq!(snapshot.highlighted, None, "都選完了，回到打字狀態");
}

#[test]
fn a_digit_with_no_candidate_does_nothing() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    let snapshot = press(&mut engine, &[Key::Down, Key::Char('9')]);
    assert_eq!(shown(&snapshot), "ㄘㄜˋ|");
    assert_eq!(snapshot.highlighted, Some(1));
    assert!(snapshot.handled);
}

#[test]
fn enter_sends_when_the_whole_sentence_guess_is_highlighted() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    pin(&mut engine, "冊");
    let snapshot = press(&mut engine, &[Key::Down, Key::Up, Key::Enter]);
    assert_eq!(snapshot.committed, "冊是");
    assert!(snapshot.is_empty());
    assert_eq!(snapshot.highlighted, None);
}

#[test]
fn enter_on_the_highlighted_guess_before_the_cursor_pins_it_and_keeps_selecting_the_rest() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    press(&mut engine, &[Key::Left, Key::Left]);
    let snapshot = press(&mut engine, &[Key::Down, Key::Up, Key::Enter]);
    assert_eq!(snapshot.committed, "");
    assert_eq!(shown(&snapshot), "[測]ㄕˋ|");
    assert_eq!(snapshot.highlighted, Some(0), "和數字鍵一樣，留在選字狀態接著選");
}

#[test]
fn enter_sends_when_the_highlighted_candidate_uses_up_every_key() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    let snapshot = press(&mut engine, &[Key::Down, Key::Enter]);
    assert_eq!(snapshot.committed, "側");
    assert!(snapshot.is_empty());
    assert_eq!(snapshot.highlighted, None);
}

#[test]
fn enter_pins_the_highlighted_candidate_when_it_is_not_the_first() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    let snapshot = press(&mut engine, &[Key::Down, Key::Down, Key::Enter]);
    assert_eq!(snapshot.committed, "");
    assert_eq!(shown(&snapshot), "[側]ㄕˋ|");
}

#[test]
fn any_other_key_returns_to_typing_and_is_handled_as_usual() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4");
    engine.handle_key(Key::Down);
    let snapshot = engine.handle_key(Key::Char('g'));
    assert_eq!(snapshot.highlighted, None);
    assert_eq!(shown(&snapshot), "ㄘㄜˋ·ㄕ|");

    engine.handle_key(Key::Down);
    let snapshot = engine.handle_key(Key::Backspace);
    assert_eq!(snapshot.highlighted, None);
    assert_eq!(shown(&snapshot), "ㄘㄜˋ|");
}

#[test]
fn space_is_the_first_tone_even_in_the_selecting_state() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4gj");
    engine.handle_key(Key::Down);
    let snapshot = engine.handle_key(Key::Char(' '));
    assert_eq!(snapshot.highlighted, None);
    assert_eq!(shown(&snapshot), "ㄘㄜˋ·ㄕㄨ |");
    assert!(snapshot.pinned.is_empty());
}

#[test]
fn clicking_a_candidate_pins_it_without_entering_the_selecting_state() {
    let mut engine = engine();
    type_keys(&mut engine, "hk4g4");
    let snapshot = engine.pin(1);
    assert_eq!(shown(&snapshot), "[測]ㄕˋ|");
    assert_eq!(snapshot.highlighted, None);
}

/// 十二個同音字，用來測試超過一頁的候選字。
fn engine_with_many_homophones() -> Engine {
    let mut builder = ModelBuilder::new();
    for (index, word) in "甲乙丙丁戊己庚辛壬癸子丑".chars().enumerate() {
        let word = word.to_string();
        assert!(builder.add_entry(&word, "ㄕˋ", 0.0));
        builder.set_unigram(&word, -2.0 - index as f32 * 0.1);
    }
    let model = Arc::new(Model::from_bytes(&builder.build()).unwrap());
    Engine::new(model, Config::default())
}

#[test]
fn zero_picks_the_tenth_candidate_on_the_page() {
    let mut engine = engine_with_many_homophones();
    assert_eq!(type_keys(&mut engine, "g4").candidates.len(), 12);
    assert_eq!(shown(&press(&mut engine, &[Key::Down, Key::Char('0')])), "[癸]|");
}

#[test]
fn page_keys_switch_pages_and_digits_pick_from_the_current_page() {
    let mut engine = engine_with_many_homophones();
    type_keys(&mut engine, "g4");
    assert_eq!(press(&mut engine, &[Key::Down, Key::PageDown]).highlighted, Some(10));
    assert_eq!(engine.handle_key(Key::PageDown).highlighted, Some(10), "沒有下一頁了");
    assert_eq!(engine.handle_key(Key::PageUp).highlighted, Some(0));
    assert_eq!(engine.handle_key(Key::PageUp).highlighted, Some(0), "沒有上一頁了");
    // ↓ 走過一頁的最後一個，就到下一頁。
    assert_eq!(press(&mut engine, &[Key::Down; 10]).highlighted, Some(10));
    assert_eq!(shown(&engine.handle_key(Key::Char('2'))), "[丑]|");
}
