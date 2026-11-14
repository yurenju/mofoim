// 網頁前端：接鍵盤事件、把按鍵交給引擎、依狀態快照畫出編輯區與候選字清單。

import { Engine, type Snapshot } from "./engine";
import { plainText, toEngineKey, type EngineKey } from "./keys";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const status = $("status");
const workspace = document.querySelector<HTMLElement>(".workspace")!;
const editor = $("editor");
const modeBadge = $("mode");
const candidateList = $<HTMLUListElement>("candidates");

/** 已經送出的文字。這個展示頁面自己扮演「應用程式」的角色。 */
let sent = "";
let snapshot: Snapshot = { cells: [], committed: "", mode: "chinese", handled: true };
/** 目前打開候選字清單的是編輯區第幾格；沒打開就是 `null`。 */
let openCell: number | null = null;
let engine: Engine;

function render() {
  editor.replaceChildren();
  editor.append(document.createTextNode(sent));
  snapshot.cells.forEach((cell, index) => {
    const selectable = cell.kind === "guess" || cell.kind === "pinned";
    const element = document.createElement("span");
    element.className = `cell ${cell.kind}`;
    element.textContent = cell.text;
    if (selectable) {
      element.dataset.cell = String(index);
      element.title = "點一下改選其他字";
    }
    editor.append(element);
  });
  const caret = document.createElement("span");
  caret.className = "caret";
  editor.append(caret);

  modeBadge.textContent = snapshot.mode === "chinese" ? "中" : "英";
  modeBadge.classList.toggle("english", snapshot.mode === "english");
  if (openCell !== null && !editor.querySelector(`[data-cell="${openCell}"]`)) openCell = null;
  renderCandidates();
}

function renderCandidates() {
  candidateList.replaceChildren();
  candidateList.hidden = openCell === null;
  if (openCell === null) return;
  const anchor = editor.querySelector<HTMLElement>(`[data-cell="${openCell}"]`)!;
  engine.candidates(openCell).forEach((word, index) => {
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.dataset.candidate = String(index);
    if (index < 9) {
      const number = document.createElement("span");
      number.className = "number";
      number.textContent = String(index + 1);
      button.append(number);
    }
    button.append(word);
    item.append(button);
    candidateList.append(item);
  });
  candidateList.style.left = `${anchor.offsetLeft}px`;
  candidateList.style.top = `${anchor.offsetTop + anchor.offsetHeight + 4}px`;
}

function apply(next: Snapshot) {
  sent += next.committed;
  snapshot = next;
  render();
}

/** 把第 `candidate` 個候選字詞釘在目前打開的那一格。 */
function choose(candidate: number) {
  if (openCell === null) return;
  const cell = openCell;
  openCell = null;
  apply(engine.pin(cell, candidate));
}

/** 引擎不處理的按鍵，由這個頁面自己當成一般的文字編輯。 */
function editPlainText(key: EngineKey) {
  if (key.kind === "enter" || key.kind === "shiftEnter") sent += "\n";
  else if (key.kind === "backspace") sent = Array.from(sent).slice(0, -1).join("");
  else sent += plainText(key);
}

/** 讓引擎的中英模式跟著 Caps Lock 的燈號走。 */
function syncMode(event: KeyboardEvent) {
  const wanted = event.getModifierState("CapsLock") ? "english" : "chinese";
  if (wanted !== snapshot.mode) apply(engine.key({ kind: "toggleMode" }));
}

editor.addEventListener("keydown", (event) => {
  const key = toEngineKey(event);
  // Caps Lock 自己的 keydown 事件裡，燈號狀態在各瀏覽器不一致，等 keyup 再同步。
  if (key?.kind === "toggleMode") return;
  syncMode(event);
  if (!key) return;
  event.preventDefault();

  if (openCell !== null) {
    const picked = key.kind === "char" ? "123456789".indexOf(key.ch) : -1;
    if (picked >= 0 && candidateList.children[picked]) return choose(picked);
    openCell = null;
    if (key.kind === "escape") return render();
  }

  const next = engine.key(key);
  if (!next.handled) editPlainText(key);
  apply(next);
});

editor.addEventListener("keyup", syncMode);

editor.addEventListener("click", (event) => {
  const cell = (event.target as HTMLElement).dataset.cell;
  const clicked = cell === undefined ? null : Number(cell);
  openCell = clicked === openCell ? null : clicked;
  renderCandidates();
});

candidateList.addEventListener("mousedown", (event) => event.preventDefault());
candidateList.addEventListener("click", (event) => {
  const button = (event.target as HTMLElement).closest<HTMLElement>("[data-candidate]");
  if (button) choose(Number(button.dataset.candidate));
});

$("clear").addEventListener("click", () => {
  sent = "";
  openCell = null;
  apply(engine.key({ kind: "escape" }));
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
