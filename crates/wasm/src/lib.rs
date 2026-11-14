//! 把引擎包成 WebAssembly 給網頁前端呼叫。
//!
//! 這一層刻意不用 wasm-bindgen 之類的工具，只匯出幾個參數全是數字的函式：
//! 前端呼叫一個操作後，再用 `mofoim_output_ptr` 與 `mofoim_output_len` 取回結果。
//! 結果一律是 UTF-8 的 JSON 字串，對應的前端包裝在 `web/src/engine.ts`。

use mofoim_engine::{CellKind, Config, Engine, Key, Mode, Model, Snapshot};
use std::sync::{Arc, Mutex};

struct State {
    engine: Option<Engine>,
    output: String,
}

static STATE: Mutex<State> = Mutex::new(State { engine: None, output: String::new() });

fn with_state(run: impl FnOnce(&mut State)) {
    let mut state = STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    run(&mut state);
}

/// 配置一塊記憶體讓前端寫入資料（例如資料檔的內容）。
#[no_mangle]
pub extern "C" fn mofoim_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len);
    let pointer = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    pointer
}

/// 釋放 `mofoim_alloc` 配置的記憶體。
///
/// # Safety
/// `pointer` 與 `len` 必須來自同一次 `mofoim_alloc` 呼叫，而且只能釋放一次。
#[no_mangle]
pub unsafe extern "C" fn mofoim_free(pointer: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(pointer, 0, len));
}

/// 用資料檔的內容建立引擎。成功回傳 1；失敗回傳 0，錯誤訊息放在結果裡。
///
/// # Safety
/// `pointer` 必須指向 `len` 個可讀的位元組。
#[no_mangle]
pub unsafe extern "C" fn mofoim_load(pointer: *const u8, len: usize) -> u32 {
    let bytes = std::slice::from_raw_parts(pointer, len);
    let mut ok = 0;
    with_state(|state| match Model::from_bytes(bytes) {
        Ok(model) => {
            state.engine = Some(Engine::new(Arc::new(model), Config::default()));
            state.output.clear();
            ok = 1;
        }
        Err(message) => state.output = message,
    });
    ok
}

/// 送一個按鍵給引擎，結果是狀態快照。`kind` 的數值見 [`key_from_code`]。
#[no_mangle]
pub extern "C" fn mofoim_key(kind: u32, ch: u32) {
    with_state(|state| {
        let (Some(engine), Some(key)) = (state.engine.as_mut(), key_from_code(kind, ch)) else {
            return state.output.clear();
        };
        state.output = snapshot_json(&engine.handle_key(key));
    });
}

/// 編輯區第 `cell` 格的候選字詞，結果是 JSON 字串陣列。
#[no_mangle]
pub extern "C" fn mofoim_candidates(cell: usize) {
    with_state(|state| {
        let words = state.engine.as_ref().map(|e| e.candidates(cell)).unwrap_or_default();
        let items: Vec<String> = words.iter().map(|w| json_string(w)).collect();
        state.output = format!("[{}]", items.join(","));
    });
}

/// 把第 `cell` 格釘住成第 `candidate` 個候選字詞，結果是狀態快照。
#[no_mangle]
pub extern "C" fn mofoim_pin(cell: usize, candidate: usize) {
    with_state(|state| {
        let Some(engine) = state.engine.as_mut() else { return state.output.clear() };
        state.output = snapshot_json(&engine.pin(cell, candidate));
    });
}

/// 不送按鍵，只取得目前的狀態快照。
#[no_mangle]
pub extern "C" fn mofoim_snapshot() {
    with_state(|state| {
        let Some(engine) = state.engine.as_ref() else { return state.output.clear() };
        state.output = snapshot_json(&engine.snapshot());
    });
}

#[no_mangle]
pub extern "C" fn mofoim_output_ptr() -> *const u8 {
    let mut pointer = std::ptr::null();
    with_state(|state| pointer = state.output.as_ptr());
    pointer
}

#[no_mangle]
pub extern "C" fn mofoim_output_len() -> usize {
    let mut len = 0;
    with_state(|state| len = state.output.len());
    len
}

/// 按鍵種類的編號：0 字元鍵、1 按住 Shift 的字元鍵、2 Enter、3 Shift+Enter、
/// 4 Backspace、5 Esc、6 切換中英。前兩種的 `ch` 是美式鍵盤沒按 Shift 時的字元。
fn key_from_code(kind: u32, ch: u32) -> Option<Key> {
    Some(match kind {
        0 => Key::Char(char::from_u32(ch)?),
        1 => Key::Shift(char::from_u32(ch)?),
        2 => Key::Enter,
        3 => Key::ShiftEnter,
        4 => Key::Backspace,
        5 => Key::Escape,
        6 => Key::ToggleMode,
        _ => return None,
    })
}

fn snapshot_json(snapshot: &Snapshot) -> String {
    let cells: Vec<String> = snapshot
        .cells
        .iter()
        .map(|cell| {
            let kind = match cell.kind {
                CellKind::Guess => "guess",
                CellKind::Pinned => "pinned",
                CellKind::Raw => "raw",
                CellKind::Literal => "literal",
            };
            format!("{{\"text\":{},\"kind\":\"{kind}\"}}", json_string(&cell.text))
        })
        .collect();
    let mode = match snapshot.mode {
        Mode::Chinese => "chinese",
        Mode::English => "english",
    };
    format!(
        "{{\"cells\":[{}],\"committed\":{},\"mode\":\"{mode}\",\"handled\":{}}}",
        cells.join(","),
        json_string(&snapshot.committed),
        snapshot.handled
    )
}

fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofoim_engine::ModelBuilder;

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(json_string("冊"), "\"冊\"");
        assert_eq!(json_string("a\"b\\c\n"), "\"a\\\"b\\\\c\\u000a\"");
    }

    #[test]
    fn key_codes_map_to_engine_keys() {
        assert_eq!(key_from_code(0, 'h' as u32), Some(Key::Char('h')));
        assert_eq!(key_from_code(1, '1' as u32), Some(Key::Shift('1')));
        assert_eq!(key_from_code(3, 0), Some(Key::ShiftEnter));
        assert_eq!(key_from_code(99, 0), None);
    }

    #[test]
    fn snapshot_json_describes_the_composition() {
        let mut builder = ModelBuilder::new();
        builder.add_entry("冊", "ㄘㄜˋ", 0.0);
        let model = Arc::new(Model::from_bytes(&builder.build()).unwrap());
        let mut engine = Engine::new(model, Config::default());
        for ch in "hk4h".chars() {
            engine.handle_key(Key::Char(ch));
        }
        assert_eq!(
            snapshot_json(&engine.snapshot()),
            r#"{"cells":[{"text":"冊","kind":"guess"},{"text":"ㄘ","kind":"raw"}],"committed":"","mode":"chinese","handled":true}"#
        );
    }
}
