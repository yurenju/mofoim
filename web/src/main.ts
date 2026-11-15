// 網頁前端：接鍵盤事件、把按鍵交給引擎、依狀態快照畫出編輯區與候選字。
//
// 游標、釘住與選字狀態都由引擎決定（見 ADR 0004），這裡只負責照狀態快照畫出來。

import { Engine, type Candidate, type Snapshot } from "./engine";
import { plainText, toEngineKey, type EngineKey } from "./keys";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const status = $("status");
const workspace = document.querySelector<HTMLElement>(".workspace")!;
const editorWrap = document.querySelector<HTMLElement>(".editor-wrap")!;
const editor = $("editor");
const modeBadge = $("mode");
const candidateWindow = $("candidates");
const candidateList = $<HTMLUListElement>("candidate-list");
const candidateHint = $("candidate-hint");

/** 候選窗一頁顯示幾個候選字，對應數字鍵 1～9、0；要和引擎的 `PAGE_SIZE` 一致。 */
const PAGE_SIZE = 10;
const PAGE_KEYS = "1234567890";

/** 已經送出的文字。這個展示頁面自己扮演「應用程式」的角色。 */
let sent = "";
let snapshot: Snapshot = {
  pinned: [],
  beforeCursor: [],
  afterCursor: [],
  candidates: [],
  highlighted: null,
  committed: "",
  mode: "chinese",
  handled: true,
};
let engine: Engine;

const span = (className: string, text = "") => {
  const element = document.createElement("span");
  element.className = className;
  element.textContent = text;
  return element;
};

/** 編輯區有沒有還沒送出的內容。 */
function isComposing(state: Snapshot) {
  return state.pinned.length + state.beforeCursor.length + state.afterCursor.length > 0;
}

/** 一串還沒釘住的注音：音節之間留空隙，一聲（空格）畫成一小段空白。 */
function zhuyin(syllables: string[], className: string) {
  const run = span(className);
  for (const syllable of syllables) run.append(span("syllable", syllable));
  return run;
}

function render() {
  editor.replaceChildren();
  editor.append(document.createTextNode(sent));

  // 釘住的段是漢字；還沒釘住的按鍵照實際按的鍵顯示成注音；游標之後的注音是灰色。
  for (const text of snapshot.pinned) editor.append(span("pinned", text));
  const unpinned = snapshot.beforeCursor.length + snapshot.afterCursor.length > 0;
  if (snapshot.pinned.length > 0 && unpinned) editor.append(span("gap"));
  const before = zhuyin(snapshot.beforeCursor, "zhuyin");
  const caret = span("caret");
  editor.append(before, caret, zhuyin(snapshot.afterCursor, "zhuyin after"));

  const english = snapshot.mode === "english";
  modeBadge.textContent = english ? "英" : "中";
  modeBadge.classList.toggle("english", english);
  modeBadge.setAttribute("aria-label", english ? "英文模式，按一下切換成中文模式" : "中文模式，按一下切換成英文模式");
  renderCandidates(snapshot.beforeCursor.length > 0 ? before : caret);
}

/** 候選字的文字；容錯改過的字另外包起來畫橘色虛線。 */
function candidateText(candidate: Candidate) {
  const text = span("text");
  Array.from(candidate.text).forEach((ch, index) => {
    text.append(candidate.corrected.includes(index) ? span("corrected", ch) : ch);
  });
  return text;
}

/** 畫出候選窗，放在 `anchor`（還沒釘住的注音的開頭）正下方。 */
function renderCandidates(anchor: HTMLElement) {
  const { candidates, highlighted } = snapshot;
  candidateList.replaceChildren();
  candidateWindow.hidden = candidates.length === 0;
  if (candidates.length === 0) return;

  // 打字狀態下數字鍵是注音，所以不顯示編號，但留著編號的位置，進入選字狀態時版面才不會跳。
  const selecting = highlighted !== null;
  const active = highlighted ?? 0;
  const pageStart = Math.floor(active / PAGE_SIZE) * PAGE_SIZE;
  candidates.slice(pageStart, pageStart + PAGE_SIZE).forEach((candidate, offset) => {
    const index = pageStart + offset;
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.dataset.candidate = String(index);
    button.classList.toggle("on", index === active);
    button.append(span("number", selecting ? PAGE_KEYS[offset] : ""), candidateText(candidate));
    item.append(button);
    candidateList.append(item);
  });

  const pages = Math.ceil(candidates.length / PAGE_SIZE);
  // 游標之後還有按鍵時，Enter 只釘住，不送出。
  const hasKeysAfterCursor = snapshot.afterCursor.length > 0;
  const hints = selecting
    ? ["數字 選字", active === 0 && !hasKeysAfterCursor ? "Enter 送出" : "Enter 選反白的", "Esc 回到打字"]
    : [hasKeysAfterCursor ? "Enter 釘住第一個" : "Enter 送出第一個", "↓ 用數字選", "滑鼠點選"];
  if (selecting && pages > 1) hints.push(`PgUp PgDn 換頁（${pageStart / PAGE_SIZE + 1}/${pages}）`);
  candidateHint.replaceChildren(...hints.map((hint) => span("", hint)));

  const left = Math.min(anchor.offsetLeft, editorWrap.clientWidth - candidateWindow.offsetWidth);
  candidateWindow.style.left = `${Math.max(0, left)}px`;
  candidateWindow.style.top = `${anchor.offsetTop + anchor.offsetHeight + 6}px`;
}

function apply(next: Snapshot) {
  sent += next.committed;
  snapshot = next;
  render();
}

/**
 * 引擎不處理的按鍵，由這個頁面自己當成一般的文字編輯。
 * 回傳這個頁面有沒有處理；沒有的話（例如方向鍵）就留給瀏覽器。
 */
function editPlainText(key: EngineKey) {
  if (key.kind === "enter" || key.kind === "shiftEnter") sent += "\n";
  else if (key.kind === "backspace") sent = Array.from(sent).slice(0, -1).join("");
  else if (key.kind === "char" || key.kind === "shift") sent += plainText(key);
  else return false;
  return true;
}

/** 切換中文模式與英文模式。 */
function toggleMode() {
  apply(engine.key({ kind: "toggleMode" }));
}

editor.addEventListener("keydown", (event) => {
  const key = toEngineKey(event);
  if (!key) return;
  // Caps Lock 可能被系統的輸入法攔走，所以畫面上另外有一個按鈕做同樣的事。
  if (key.kind === "toggleMode") {
    event.preventDefault();
    if (!event.repeat) toggleMode();
    return;
  }

  // 編輯區有內容時，所有按鍵都屬於輸入法：Enter 只送出、不會再多加一個換行。
  const composing = isComposing(snapshot);
  const next = engine.key(key);
  if (next.handled || composing || editPlainText(key)) event.preventDefault();
  apply(next);
});

// 點候選字不搶走打字區的焦點，選完可以直接繼續打字。
candidateWindow.addEventListener("mousedown", (event) => event.preventDefault());
candidateList.addEventListener("click", (event) => {
  const button = (event.target as HTMLElement).closest<HTMLElement>("[data-candidate]");
  if (button) apply(engine.pin(Number(button.dataset.candidate)));
});

// 按鈕不搶走打字區的焦點，切換後可以直接繼續打字。
modeBadge.addEventListener("mousedown", (event) => event.preventDefault());
modeBadge.addEventListener("click", () => {
  toggleMode();
  editor.focus();
});

$("clear").addEventListener("click", () => {
  sent = "";
  // 選字狀態下第一次 Esc 只會回到打字狀態，所以最多要按兩次才清得乾淨。
  let next = engine.key({ kind: "escape" });
  if (isComposing(next)) next = engine.key({ kind: "escape" });
  apply(next);
  editor.focus();
});

/** 畫出大千鍵盤的對照圖。 */
function renderKeyboard() {
  const rows = [
    ["1ㄅ", "2ㄉ", "3ˇ", "4ˋ", "5ㄓ", "6ˊ", "7˙", "8ㄚ", "9ㄞ", "0ㄢ", "-ㄦ"],
    ["qㄆ", "wㄊ", "eㄍ", "rㄐ", "tㄔ", "yㄗ", "uㄧ", "iㄛ", "oㄟ", "pㄣ"],
    ["aㄇ", "sㄋ", "dㄎ", "fㄑ", "gㄕ", "hㄘ", "jㄨ", "kㄜ", "lㄠ", ";ㄤ"],
    ["zㄈ", "xㄌ", "cㄏ", "vㄒ", "bㄖ", "nㄙ", "mㄩ", ",ㄝ", ".ㄡ", "/ㄥ"],
  ];
  const keyboard = $("keyboard");
  for (const row of rows) {
    const line = document.createElement("div");
    line.className = "row";
    for (const key of row) {
      const cap = document.createElement("span");
      cap.className = "key";
      const symbol = document.createElement("b");
      symbol.textContent = key.slice(1);
      cap.append(key[0].toUpperCase(), symbol);
      line.append(cap);
    }
    keyboard.append(line);
  }
  const space = document.createElement("div");
  space.className = "row";
  space.innerHTML = '<span class="key space">空白鍵<b>一聲</b></span>';
  keyboard.append(space);
}

async function start() {
  const response = await fetch(`${import.meta.env.BASE_URL}mofoim.dat`);
  if (!response.ok) throw new Error(`下載資料檔失敗（${response.status}）`);
  engine = await Engine.load(new Uint8Array(await response.arrayBuffer()));
  snapshot = engine.snapshot();
  status.hidden = true;
  workspace.hidden = false;
  renderKeyboard();
  render();
  editor.focus();
}

start().catch((error) => {
  status.textContent = `載入失敗：${error instanceof Error ? error.message : error}`;
  status.classList.add("error");
});
