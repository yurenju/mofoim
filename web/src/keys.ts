// 把瀏覽器的鍵盤事件翻成引擎認得的抽象按鍵。
//
// 一律用 `event.code`（實體鍵的位置）而不是 `event.key`（打出來的字元），
// 這樣不管作業系統的鍵盤語言或 Caps Lock 狀態如何，引擎拿到的都是同一個鍵。

export type EngineKey =
  | { kind: "char" | "shift"; ch: string }
  | { kind: "enter" | "shiftEnter" | "backspace" | "escape" | "toggleMode" };

/** `KeyboardEvent` 裡這個模組會用到的欄位。 */
export interface KeyPress {
  code: string;
  shiftKey: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
}

/** 實體鍵對應到美式鍵盤「沒按 Shift 時」的字元。 */
const CODE_TO_CHAR: Record<string, string> = {
  Space: " ",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
};

const PLAIN = "`1234567890-=[]\\;',./";
const SHIFTED = '~!@#$%^&*()_+{}|:"<>?';

/** 這個實體鍵代表的字元；不是字元鍵就回傳 `undefined`。 */
function charForCode(code: string): string | undefined {
  if (/^Key[A-Z]$/.test(code)) return code[3].toLowerCase();
  if (/^Digit[0-9]$/.test(code)) return code[5];
  return CODE_TO_CHAR[code];
}

/** 把鍵盤事件翻成抽象按鍵；引擎不需要知道的按鍵（方向鍵、快速鍵等）回傳 `null`。 */
export function toEngineKey(press: KeyPress): EngineKey | null {
  if (press.ctrlKey || press.altKey || press.metaKey) return null;
  switch (press.code) {
    case "Enter":
    case "NumpadEnter":
      return { kind: press.shiftKey ? "shiftEnter" : "enter" };
    case "Backspace":
      return { kind: "backspace" };
    case "Escape":
      return { kind: "escape" };
    case "CapsLock":
      return { kind: "toggleMode" };
  }
  const ch = charForCode(press.code);
  if (ch === undefined) return null;
  return { kind: press.shiftKey ? "shift" : "char", ch };
}

/** 引擎不處理某個字元鍵時，應用程式本來會收到的字元。 */
export function plainText(key: EngineKey): string {
  if (key.kind === "char") return key.ch;
  if (key.kind !== "shift") return "";
  const index = PLAIN.indexOf(key.ch);
  return index >= 0 ? SHIFTED[index] : key.ch.toUpperCase();
}
