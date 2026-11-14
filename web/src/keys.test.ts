import { describe, expect, it } from "vitest";
import { plainText, toEngineKey } from "./keys";

const press = (code: string, extra: Partial<Parameters<typeof toEngineKey>[0]> = {}) =>
  toEngineKey({ code, shiftKey: false, ...extra });

describe("toEngineKey", () => {
  it("用實體鍵的位置決定字元，不受 Caps Lock 或鍵盤語言影響", () => {
    expect(press("KeyH")).toEqual({ kind: "char", ch: "h" });
    expect(press("Digit4")).toEqual({ kind: "char", ch: "4" });
    expect(press("Space")).toEqual({ kind: "char", ch: " " });
    expect(press("Comma")).toEqual({ kind: "char", ch: "," });
    expect(press("Backslash")).toEqual({ kind: "char", ch: "\\" });
  });

  it("按住 Shift 的字元鍵是另一種按鍵", () => {
    expect(press("KeyA", { shiftKey: true })).toEqual({ kind: "shift", ch: "a" });
    expect(press("Digit1", { shiftKey: true })).toEqual({ kind: "shift", ch: "1" });
  });

  it("認得編輯用的按鍵", () => {
    expect(press("Enter")).toEqual({ kind: "enter" });
    expect(press("Enter", { shiftKey: true })).toEqual({ kind: "shiftEnter" });
    expect(press("Backspace")).toEqual({ kind: "backspace" });
    expect(press("Escape")).toEqual({ kind: "escape" });
    expect(press("CapsLock")).toEqual({ kind: "toggleMode" });
  });

  it("方向鍵與 End 移動游標", () => {
    expect(press("ArrowLeft")).toEqual({ kind: "left" });
    expect(press("ArrowRight")).toEqual({ kind: "right" });
    expect(press("End")).toEqual({ kind: "end" });
  });

  it("Ctrl 加左右鍵一次移一個音節", () => {
    expect(press("ArrowLeft", { ctrlKey: true })).toEqual({ kind: "syllableLeft" });
    expect(press("ArrowRight", { ctrlKey: true })).toEqual({ kind: "syllableRight" });
  });

  it("上下鍵與翻頁鍵交給引擎的選字狀態", () => {
    expect(press("ArrowDown")).toEqual({ kind: "down" });
    expect(press("ArrowUp")).toEqual({ kind: "up" });
    expect(press("PageDown")).toEqual({ kind: "pageDown" });
    expect(press("PageUp")).toEqual({ kind: "pageUp" });
  });

  it("空白鍵和數字鍵永遠是字元鍵，要不要當成選字由引擎決定", () => {
    expect(press("Space")).toEqual({ kind: "char", ch: " " });
    expect(press("Digit1")).toEqual({ kind: "char", ch: "1" });
    expect(press("Digit0")).toEqual({ kind: "char", ch: "0" });
  });

  it("不攔截快速鍵和引擎用不到的按鍵", () => {
    expect(press("KeyC", { ctrlKey: true })).toBeNull();
    expect(press("KeyV", { metaKey: true })).toBeNull();
    expect(press("ArrowDown", { ctrlKey: true })).toBeNull();
    expect(press("End", { ctrlKey: true })).toBeNull();
    expect(press("ArrowLeft", { altKey: true })).toBeNull();
    expect(press("ArrowLeft", { metaKey: true })).toBeNull();
    expect(press("Home")).toBeNull();
    expect(press("F5")).toBeNull();
  });
});

describe("plainText", () => {
  it("回傳沒有輸入法時這個鍵會打出的字元", () => {
    expect(plainText({ kind: "char", ch: "h" })).toBe("h");
    expect(plainText({ kind: "shift", ch: "h" })).toBe("H");
    expect(plainText({ kind: "shift", ch: "1" })).toBe("!");
    expect(plainText({ kind: "shift", ch: "/" })).toBe("?");
    expect(plainText({ kind: "enter" })).toBe("");
    expect(plainText({ kind: "left" })).toBe("");
  });
});
