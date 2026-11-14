// 引擎的前端包裝：載入 WebAssembly 與資料檔，把引擎的操作包成一般的函式。

import wasmUrl from "./wasm/mofoim.wasm?url";
import type { EngineKey } from "./keys";

/** 一個候選字。 */
export interface Candidate {
  text: string;
  /** `text` 裡第幾個字是容錯改過的；只有整句猜測會標示。 */
  corrected: number[];
}

/** 每次操作後引擎回傳的完整狀態。 */
export interface Snapshot {
  /** 釘住的段，由前到後，每一段是使用者選定的文字。 */
  pinned: string[];
  /** 游標之前、還沒釘住的按鍵：一個音節一個字串、一個按鍵一個字元，一聲是空格。 */
  beforeCursor: string[];
  /** 游標之後的按鍵，格式同上。 */
  afterCursor: string[];
  /** 候選字。第一個是整句猜測，其餘是只涵蓋前面幾個音節的較短字詞。 */
  candidates: Candidate[];
  /** 選字狀態下反白的是第幾個候選字；`null` 代表打字狀態。 */
  highlighted: number | null;
  /** 這次操作要送出的文字。 */
  committed: string;
  mode: "chinese" | "english";
  /** 引擎有沒有處理這個按鍵；沒有的話前端要自己處理。 */
  handled: boolean;
}

interface Exports {
  memory: WebAssembly.Memory;
  mofoim_alloc(len: number): number;
  mofoim_free(pointer: number, len: number): void;
  mofoim_load(pointer: number, len: number): number;
  mofoim_key(kind: number, ch: number): void;
  mofoim_pin(candidate: number): void;
  mofoim_snapshot(): void;
  mofoim_output_ptr(): number;
  mofoim_output_len(): number;
}

/** 按鍵種類在引擎那一側的編號，要和 `crates/wasm/src/lib.rs` 一致。 */
const KEY_CODES: Record<EngineKey["kind"], number> = {
  char: 0,
  shift: 1,
  enter: 2,
  shiftEnter: 3,
  backspace: 4,
  escape: 5,
  toggleMode: 6,
  left: 7,
  right: 8,
  syllableLeft: 9,
  syllableRight: 10,
  end: 11,
  down: 12,
  up: 13,
  pageDown: 14,
  pageUp: 15,
};

export class Engine {
  private constructor(private readonly wasm: Exports) {}

  /** 載入引擎與資料檔。`data` 是資料檔的內容。 */
  static async load(data: Uint8Array): Promise<Engine> {
    const { instance } = await WebAssembly.instantiateStreaming(fetch(wasmUrl));
    const wasm = instance.exports as unknown as Exports;
    const pointer = wasm.mofoim_alloc(data.length);
    new Uint8Array(wasm.memory.buffer, pointer, data.length).set(data);
    const ok = wasm.mofoim_load(pointer, data.length);
    wasm.mofoim_free(pointer, data.length);
    const engine = new Engine(wasm);
    if (!ok) throw new Error(engine.output());
    return engine;
  }

  key(key: EngineKey): Snapshot {
    const ch = "ch" in key ? key.ch.codePointAt(0)! : 0;
    this.wasm.mofoim_key(KEY_CODES[key.kind], ch);
    return JSON.parse(this.output());
  }

  /** 釘住第 `candidate` 個候選字（滑鼠點選用）。 */
  pin(candidate: number): Snapshot {
    this.wasm.mofoim_pin(candidate);
    return JSON.parse(this.output());
  }

  snapshot(): Snapshot {
    this.wasm.mofoim_snapshot();
    return JSON.parse(this.output());
  }

  private output(): string {
    const bytes = new Uint8Array(
      this.wasm.memory.buffer,
      this.wasm.mofoim_output_ptr(),
      this.wasm.mofoim_output_len(),
    );
    return new TextDecoder().decode(bytes);
  }
}
