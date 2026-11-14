//! 鍵盤配置：每個鍵對應哪個注音符號，以及哪些鍵彼此相鄰。

use crate::syllable::Symbol;

/// 描述一種鍵盤配置的原始資料。新增配置只要再寫一份這個結構。
pub struct LayoutSpec {
    pub name: &'static str,
    /// 實體鍵盤由上到下的每一列，以美式鍵盤未按 Shift 時的字元代表每個鍵。
    pub rows: &'static [&'static str],
    /// 每一列相對於第一列往右錯開多少個鍵寬。
    pub row_offsets: &'static [f32],
    /// 鍵與注音符號的對應，兩個字元一組：鍵、符號。
    pub symbols: &'static str,
    /// 代表一聲的鍵。
    pub first_tone_key: char,
}

/// 大千（標準）注音鍵盤。
pub const DACHEN: LayoutSpec = LayoutSpec {
    name: "大千",
    rows: &["1234567890-", "qwertyuiop", "asdfghjkl;", "zxcvbnm,./"],
    row_offsets: &[0.0, 0.5, 0.75, 1.25],
    symbols: "1ㄅqㄆaㄇzㄈ2ㄉwㄊsㄋxㄌeㄍdㄎcㄏrㄐfㄑvㄒ5ㄓtㄔgㄕbㄖyㄗhㄘnㄙ\
              uㄧjㄨmㄩ8ㄚiㄛkㄜ,ㄝ9ㄞoㄟlㄠ.ㄡ0ㄢpㄣ;ㄤ/ㄥ-ㄦ6ˊ3ˇ4ˋ7˙",
    first_tone_key: ' ',
};

/// 上下兩列的鍵，水平距離在這個範圍內就算相鄰。
const ADJACENT_REACH: f32 = 0.8;

pub struct Layout {
    pub name: &'static str,
    symbols: [Option<Symbol>; 128],
    neighbors: Vec<Vec<char>>,
}

impl Layout {
    pub fn dachen() -> Layout {
        Layout::from_spec(&DACHEN)
    }

    pub fn from_spec(spec: &LayoutSpec) -> Layout {
        let mut symbols = [None; 128];
        let chars: Vec<char> = spec.symbols.chars().filter(|c| !c.is_whitespace()).collect();
        for pair in chars.chunks(2) {
            let symbol = Symbol::from_char(pair[1]).expect("鍵盤配置裡有不認得的注音符號");
            symbols[pair[0] as usize] = Some(symbol);
        }
        symbols[spec.first_tone_key as usize] = Some(Symbol::FIRST_TONE);

        // 只有對應到注音符號的鍵才會被當成相鄰鍵。一聲鍵（空白鍵）不在任何一列裡，
        // 所以它不是任何鍵的相鄰鍵，也沒有相鄰鍵。
        let mut placed: Vec<(char, usize, f32)> = Vec::new();
        for (row, keys) in spec.rows.iter().enumerate() {
            for (col, key) in keys.chars().enumerate() {
                if symbols[key as usize].is_some() {
                    placed.push((key, row, col as f32 + spec.row_offsets[row]));
                }
            }
        }
        let mut neighbors = vec![Vec::new(); 128];
        for &(key, row, x) in &placed {
            for &(other, other_row, other_x) in &placed {
                let dx = (x - other_x).abs();
                let adjacent = match row.abs_diff(other_row) {
                    0 => other != key && dx < 1.5,
                    1 => dx < ADJACENT_REACH,
                    _ => false,
                };
                if adjacent {
                    neighbors[key as usize].push(other);
                }
            }
        }
        Layout { name: spec.name, symbols, neighbors }
    }

    /// 這個鍵對應的注音符號；沒有對應就是 `None`。
    pub fn symbol(&self, key: char) -> Option<Symbol> {
        self.symbols.get(key as usize).copied().flatten()
    }

    /// 這個鍵的相鄰鍵。
    pub fn neighbors(&self, key: char) -> &[char] {
        self.neighbors.get(key as usize).map_or(&[], |v| v.as_slice())
    }

    /// 打出這個符號要按哪個鍵。
    pub fn key_for(&self, symbol: Symbol) -> Option<char> {
        (0u8..128).map(char::from).find(|&k| self.symbol(k) == Some(symbol))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syllable::Slot;

    fn sorted(keys: &[char]) -> String {
        let mut v = keys.to_vec();
        v.sort();
        v.into_iter().collect()
    }

    #[test]
    fn dachen_maps_keys_to_symbols() {
        let layout = Layout::dachen();
        let shown = |key| layout.symbol(key).and_then(Symbol::to_char);
        assert_eq!(shown('h'), Some('ㄘ'));
        assert_eq!(shown('k'), Some('ㄜ'));
        assert_eq!(shown('4'), Some('ˋ'));
        assert_eq!(layout.symbol(' '), Some(Symbol::FIRST_TONE));
        assert_eq!(layout.symbol('='), None);
    }

    #[test]
    fn every_symbol_has_exactly_one_key() {
        let layout = Layout::dachen();
        let mapped = (0u8..128).filter(|&k| layout.symbol(k as char).is_some()).count();
        assert_eq!(mapped, 21 + 3 + 13 + 5);
    }

    #[test]
    fn neighbors_follow_the_physical_keyboard() {
        let layout = Layout::dachen();
        assert_eq!(sorted(layout.neighbors('k')), sorted(&['j', 'l', 'i', 'o', 'm', ',']));
        assert_eq!(sorted(layout.neighbors('4')), sorted(&['3', '5', 'e', 'r']));
        assert_eq!(sorted(layout.neighbors('q')), sorted(&['1', '2', 'w', 'a']));
    }

    #[test]
    fn neighbors_are_symmetric() {
        let layout = Layout::dachen();
        for key in (0u8..128).map(char::from) {
            for &other in layout.neighbors(key) {
                assert!(layout.neighbors(other).contains(&key), "{key} 與 {other}");
            }
        }
    }

    #[test]
    fn first_tone_key_is_nobodys_neighbor() {
        let layout = Layout::dachen();
        assert!(layout.neighbors(' ').is_empty());
        for key in (0u8..128).map(char::from) {
            assert!(!layout.neighbors(key).contains(&' '));
        }
    }

    #[test]
    fn key_for_is_the_inverse_of_symbol() {
        let layout = Layout::dachen();
        let ce = Symbol { slot: Slot::Initial, index: 20 };
        assert_eq!(layout.key_for(ce), Some('h'));
        assert_eq!(layout.key_for(Symbol::FIRST_TONE), Some(' '));
    }
}
