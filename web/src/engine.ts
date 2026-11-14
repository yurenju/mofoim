// 引擎的前端包裝：載入 WebAssembly 與資料檔，把引擎的操作包成一般的函式。

import wasmUrl from "./wasm/mofoim.wasm?url";
import type { EngineKey } from "./keys";

export type CellKind = "guess" | "pinned" | "raw" | "literal";

/** 編輯區裡的一格：一個漢字、一個注音符號，或一個原樣字元。 */
export interface Cell {
  text: string;
  kind: CellKind;
}

/** 每次操作後引擎回傳的完整狀態。 */
export interface Snapshot {
  cells: Cell[];
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
  mofoim_candidates(cell: number): void;
  mofoim_pin(cell: number, candidate: number): void;
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

  /** 編輯區第 `cell` 格可以選的候選字詞。 */
  candidates(cell: number): string[] {
    this.wasm.mofoim_candidates(cell);
    return JSON.parse(this.output());
  }

  /** 把第 `cell` 格釘住成第 `candidate` 個候選字詞。 */
  pin(cell: number, candidate: number): Snapshot {
    this.wasm.mofoim_pin(cell, candidate);
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
