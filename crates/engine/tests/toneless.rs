//! 引擎行為測試：不打聲調。沒有聲調鍵的音節，引擎依前後文猜出它是哪個聲調（見 ADR 0005）。

mod common;

use common::{candidates, engine, exact_engine, pin, press, shown, toned_engine, type_keys, typed};
use mofoim_engine::Key;

#[test]
fn a_syllable_without_a_tone_is_read_as_its_most_likely_tone() {
    assert_eq!(typed("hk"), "測", "ㄘㄜ 只可能是四聲");
    assert_eq!(typed("g"), "是", "ㄕ 單獨一個符號也可以是一個字");
    assert_eq!(typed("gj"), "書", "一聲不必按空白鍵");
}

#[test]
fn a_whole_sentence_can_be_typed_without_any_tone() {
    assert_eq!(typed("g.hk"), "手冊");
    assert_eq!(typed("u1pgj"), "一本書");
    assert_eq!(typed("5j/jp"), "中文");
}

#[test]
fn syllables_with_and_without_tones_can_be_mixed() {
    assert_eq!(typed("g.3hk"), "手冊");
    assert_eq!(typed("g.hk4"), "手冊");
    assert_eq!(typed("u 1pg.3hk"), "一本手冊");
}

#[test]
fn a_tone_that_was_typed_is_respected() {
    // ㄘㄠ 沒打聲調時是「草」；打了四聲就不是，引擎改猜成按錯的「測」。
    assert_eq!(typed("hl"), "草");
    assert_eq!(typed("hl4"), "測");
}

#[test]
fn context_picks_the_tone() {
    // 單打 ㄘㄜ 是「測」，接在「手」後面要變成「冊」。
    assert_eq!(typed("hk"), "測");
    assert_eq!(typed("g.hk"), "手冊");
}

#[test]
fn the_editing_area_still_shows_only_the_keys_pressed() {
    let snapshot = type_keys(&mut engine(), "g.hk");
    assert_eq!(shown(&snapshot), "ㄕㄡ·ㄘㄜ|", "不會替使用者補上聲調");
    assert_eq!(snapshot.candidates[0].text, "手冊");
}

#[test]
fn symbols_must_be_in_order_when_the_tone_is_left_out() {
    // 有聲調鍵標出結尾時順序不拘；沒有時，顛倒的 ㄜㄘ 不算是 ㄘㄜ。
    assert_eq!(type_keys(&mut exact_engine(), "kh4").text(), "測");
    assert_eq!(type_keys(&mut exact_engine(), "kh").text(), "ㄜㄘ");
}

#[test]
fn a_neighboring_key_is_still_recovered_without_a_tone() {
    // 想打 ㄕㄡ（g.），. 按成隔壁的 ,；ㄕㄝ 不在詞庫裡。
    assert_eq!(typed("g,"), "手");
    assert_eq!(type_keys(&mut exact_engine(), "g,").text(), "ㄕㄝ");
}

#[test]
fn tones_can_be_required() {
    assert_eq!(type_keys(&mut toned_engine(), "g.hk").text(), "ㄕㄡㄘㄜ");
    assert_eq!(type_keys(&mut toned_engine(), "g.3hk4").text(), "手冊");
}

#[test]
fn candidates_never_cut_a_syllable_off_from_its_tone() {
    // ㄘㄜˋ 的「測」只有涵蓋到聲調的那一個，不會多一個只涵蓋 ㄘㄜ 的「測」。
    let mut engine = engine();
    let snapshot = type_keys(&mut engine, "hk4g4");
    let shown_candidates = candidates(&snapshot);
    assert_eq!(shown_candidates[..4], ["測試", "測", "側", "冊"]);
    assert_eq!(shown_candidates.iter().filter(|text| **text == "測").count(), 1);
    assert_eq!(shown(&pin(&mut engine, "測")), "[測]ㄕˋ|");
}

#[test]
fn candidates_never_cut_a_syllable_off_from_its_later_symbols() {
    // ㄕㄨ 的候選字都要涵蓋到 ㄨ，不能只拿 ㄕ 去當「是」。
    let snapshot = type_keys(&mut engine(), "gj");
    assert_eq!(candidates(&snapshot), ["書", "輸"]);
}

#[test]
fn a_shorter_candidate_can_end_where_the_next_syllable_clearly_begins() {
    // ㄘㄜㄕ：ㄕ 是另一個聲母，所以「測」可以單獨釘住。
    let mut engine = engine();
    type_keys(&mut engine, "hkg");
    assert_eq!(shown(&pin(&mut engine, "測")), "[測]ㄕ|");
}

#[test]
fn keys_without_tones_are_grouped_where_a_new_syllable_begins() {
    let mut engine = engine();
    type_keys(&mut engine, "g.hk");
    // 游標之後沒有猜測可以依據：又出現一個聲母，就當成下一個音節。
    let snapshot = press(&mut engine, &[Key::Left; 4]);
    assert_eq!(shown(&snapshot), "|ㄕㄡ·ㄘㄜ");
    assert_eq!(shown(&engine.handle_key(Key::SyllableRight)), "ㄕㄡ|ㄘㄜ");
    assert_eq!(shown(&engine.handle_key(Key::SyllableRight)), "ㄕㄡ·ㄘㄜ|");
    assert_eq!(shown(&engine.handle_key(Key::SyllableLeft)), "ㄕㄡ|ㄘㄜ");
}

#[test]
fn space_after_symbols_is_still_the_first_tone() {
    let snapshot = type_keys(&mut engine(), "gj hk");
    assert_eq!(shown(&snapshot), "ㄕㄨ ·ㄘㄜ|");
    assert_eq!(snapshot.text(), "書測");
}

#[test]
fn enter_sends_the_guess_for_syllables_without_tones() {
    let mut engine = engine();
    type_keys(&mut engine, "jig");
    assert_eq!(engine.handle_key(Key::Enter).committed, "我是");
}
