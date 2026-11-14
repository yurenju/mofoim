# 引擎用 Rust 寫，所有平台共用同一份

mofoim 要在網頁、Windows（TSF）與 macOS（InputMethodKit）上執行，優先順序是 Windows > macOS，但第一版先做網頁。我們決定引擎用 Rust 寫成一個不碰螢幕、檔案、網路與時間的函式庫：網頁版編譯成 WebAssembly，原生平台編譯成原生函式庫，各平台的前端只負責把平台鍵碼翻成抽象按鍵、顯示引擎回傳的狀態快照。

## Considered Options

- **TypeScript**：網頁版最快做出來，但 Windows 輸入法是會被載入到每個應用程式裡的 DLL，在裡面嵌入 JS runtime 的體積與啟動成本不可接受。
- **C++**：對 TSF 最直接，但編譯成 WebAssembly 與撰寫測試的成本都比 Rust 高。

## Consequences

- 詞庫與語言模型由前端以位元組交給引擎，引擎本身不讀檔。
- 引擎的行為只取決於收到的按鍵與資料，測試可以用「餵按鍵、檢查快照」的方式寫，結果固定。
