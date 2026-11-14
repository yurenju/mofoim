//! 引擎行為測試：容錯。使用者按到相鄰鍵時，引擎依前後文猜出原本想打的字。

mod common;

use common::{engine, exact_engine, type_keys, typed};
use mofoim_engine::CellKind;

#[test]
fn a_neighboring_key_is_read_as_the_intended_one() {
    // 想打 ㄘㄜˋ（hk4），k 按成隔壁的 l，變成詞庫裡沒有的 ㄘㄠˋ。
    assert_eq!(typed("hl4"), "測");
    // 聲母按錯：h 按成隔壁的 g，ㄕㄜˋ 也不在詞庫裡。
    assert_eq!(typed("gk4"), "測");
}

#[test]
fn without_tolerance_the_same_keys_stay_as_symbols() {
    assert_eq!(type_keys(&mut exact_engine(), "hl4").text(), "ㄘㄠˋ");
}

#[test]
fn a_tone_key_hit_as_its_neighbor_still_ends_the_syllable() {
    // 4（ˋ）按成隔壁的 e（ㄍ）：引擎看不到聲調，仍然要認出這是一個字。
    let snapshot = type_keys(&mut engine(), "hke");
    assert_eq!(snapshot.text(), "測");
    assert_eq!(snapshot.cells[0].kind, CellKind::Guess);
    assert_eq!(typed("g.3hke"), "手冊");
}

#[test]
fn a_symbol_key_hit_as_a_tone_key_does_not_split_the_syllable() {
    // 想打 ㄍㄜˋ（ek4），e 按成隔壁的 4。
    assert_eq!(typed("4k4"), "個");
    assert_eq!(typed("ji3g44k4"), "我是個");
}

#[test]
fn two_wrong_keys_in_one_syllable_are_recovered_when_nothing_else_fits() {
    // hk4 的後兩個鍵都按錯，變成 hl5；ㄘㄠ 加上 ㄓ 讀不成任何字。
    assert_eq!(typed("hl5"), "測");
}

#[test]
fn a_plausible_literal_reading_wins_over_a_typo_reading() {
    // hl3 照字面是 ㄘㄠˇ（草），本身說得通，就不該被改掉。
    assert_eq!(typed("hl3"), "草");
    assert_eq!(typed("g.3hl3"), "手草");
}

#[test]
fn context_can_outweigh_a_plausible_literal_reading() {
    // 同樣的 hl3，前面是「一本」時，「手冊」比「手草」合理得多。
    assert_eq!(typed("u 1p3g.3hl3"), "一本手冊");
}

#[test]
fn exact_typing_gives_the_same_result_with_and_without_tolerance() {
    for keys in ["hk4", "g.3hk4", "u 1p3gj ", "hk4g4", "ji3g4", "5j/ jp6283y4"] {
        assert_eq!(typed(keys), type_keys(&mut exact_engine(), keys).text(), "{keys}");
    }
}

#[test]
fn keys_that_fit_nothing_even_with_tolerance_stay_visible() {
    assert_eq!(typed("hh"), "ㄘㄘ");
    assert_eq!(typed("ji3hh"), "我ㄘㄘ");
}

#[test]
fn the_first_tone_key_is_never_read_as_another_tone() {
    // 空白鍵沒有相鄰鍵：ㄕㄡ 加空白只能是一聲，不會被當成按錯的 ˇ 而變成「手」。
    assert_eq!(typed("g. "), "ㄕㄡ ");
}
