//! 注音符號與音節。

/// 注音符號在一個音節裡的位置：聲母、介音、韻母、聲調，打字時依這個順序出現。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Slot {
    Initial = 0,
    Medial = 1,
    Final = 2,
    Tone = 3,
}

const INITIALS: &str = "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙ";
const MEDIALS: &str = "ㄧㄨㄩ";
const FINALS: &str = "ㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ";
/// 聲調編號 2 到 5 對應的符號；一聲（編號 1）沒有符號。
const TONES: &str = "ˊˇˋ˙";

/// 一個注音符號。`index` 從 1 開始，0 保留給「這個位置沒有符號」。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Symbol {
    pub slot: Slot,
    pub index: u8,
}

impl Symbol {
    pub const FIRST_TONE: Symbol = Symbol { slot: Slot::Tone, index: 1 };

    pub fn from_char(ch: char) -> Option<Symbol> {
        let find = |table: &str, slot: Slot, base: u8| {
            table
                .chars()
                .position(|c| c == ch)
                .map(|i| Symbol { slot, index: i as u8 + base })
        };
        find(INITIALS, Slot::Initial, 1)
            .or_else(|| find(MEDIALS, Slot::Medial, 1))
            .or_else(|| find(FINALS, Slot::Final, 1))
            .or_else(|| find(TONES, Slot::Tone, 2))
    }

    /// 顯示用的字元；一聲沒有符號，回傳 `None`。
    pub fn to_char(self) -> Option<char> {
        let i = self.index as usize;
        match self.slot {
            Slot::Initial => INITIALS.chars().nth(i - 1),
            Slot::Medial => MEDIALS.chars().nth(i - 1),
            Slot::Final => FINALS.chars().nth(i - 1),
            Slot::Tone => {
                if i < 2 {
                    None
                } else {
                    TONES.chars().nth(i - 2)
                }
            }
        }
    }
}

/// 一個帶聲調的音節，壓成 14 個位元：聲母 5、介音 2、韻母 4、聲調 3。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Syllable(pub u16);

impl Syllable {
    /// 壓縮後數值的上限（不含），可用來配置查表陣列。
    pub const LIMIT: usize = 1 << 14;

    /// `parts` 依序是聲母、介音、韻母、聲調的 `index`。至少要有一個非聲調符號，且要有聲調。
    pub fn new(parts: [u8; 4]) -> Option<Syllable> {
        let [i, m, f, t] = parts;
        if (i == 0 && m == 0 && f == 0) || t == 0 {
            return None;
        }
        Some(Syllable(
            (i as u16) << 9 | (m as u16) << 7 | (f as u16) << 3 | t as u16,
        ))
    }

    /// 聲調不拘的音節：只有聲母、介音、韻母（`parts` 裡的聲調會被忽略）。
    /// 用來一次查出「不管哪個聲調」的所有讀音，不代表任何實際的讀音。
    pub fn any_tone(parts: [u8; 4]) -> Option<Syllable> {
        Syllable::new([parts[0], parts[1], parts[2], 1]).map(|syllable| Syllable(syllable.0 & !7))
    }

    /// 這個音節有沒有確定的聲調；[`Syllable::any_tone`] 做出來的沒有。
    pub fn has_tone(self) -> bool {
        self.0 & 7 != 0
    }

    pub fn parts(self) -> [u8; 4] {
        let v = self.0;
        [(v >> 9) as u8, (v >> 7 & 3) as u8, (v >> 3 & 15) as u8, (v & 7) as u8]
    }

    /// 解析像「ㄘㄜˋ」這樣的字串。沒有聲調符號就是一聲。
    pub fn parse(text: &str) -> Option<Syllable> {
        let mut parts = [0u8; 4];
        for ch in text.chars() {
            let sym = Symbol::from_char(ch)?;
            let slot = sym.slot as usize;
            if parts[slot] != 0 {
                return None;
            }
            parts[slot] = sym.index;
        }
        if parts[3] == 0 {
            parts[3] = 1;
        }
        Syllable::new(parts)
    }

    /// 依打字順序列出這個音節的符號（一聲也包含在內）。
    pub fn symbols(self) -> Vec<Symbol> {
        let slots = [Slot::Initial, Slot::Medial, Slot::Final, Slot::Tone];
        self.parts()
            .iter()
            .zip(slots)
            .filter(|(index, _)| **index != 0)
            .map(|(index, slot)| Symbol { slot, index: *index })
            .collect()
    }
}

impl std::fmt::Display for Syllable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for sym in self.symbols() {
            if let Some(ch) = sym.to_char() {
                write!(f, "{ch}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display_round_trip() {
        for text in ["ㄘㄜˋ", "ㄘㄠˇ", "ㄓㄨㄥ", "ㄉㄜ˙", "ㄦˊ", "ㄩㄢˊ"] {
            assert_eq!(Syllable::parse(text).unwrap().to_string(), text);
        }
    }

    #[test]
    fn no_tone_mark_means_first_tone() {
        assert_eq!(Syllable::parse("ㄓㄨㄥ").unwrap().parts()[3], 1);
    }

    #[test]
    fn rejects_malformed_text() {
        assert!(Syllable::parse("ˋ").is_none(), "只有聲調");
        assert!(Syllable::parse("ㄘㄙ").is_none(), "兩個聲母");
        assert!(Syllable::parse("冊").is_none(), "不是注音符號");
    }
}
