# mofoim

mofoim 是一個會容錯的注音輸入法：按到隔壁鍵時，它依前後文猜出你原本想打的句子。
完全在本機執行，不需要網路。

目前有引擎和網頁版的展示頁面；Windows 與 macOS 的輸入法是之後的工作。

名詞的定義見 [GLOSSARY.md](GLOSSARY.md)，重要的設計決定見 [docs/adr/](docs/adr/)。

## 目錄

| 路徑 | 內容 |
|---|---|
| `crates/engine` | 引擎：收抽象按鍵，回傳編輯區的狀態快照。不依賴任何平台 |
| `crates/data-builder` | 建置工具：把上游的詞庫與語言模型轉成引擎用的資料檔 |
| `crates/accuracy` | 準確率測試：模擬打字（可加入誤觸），量測引擎猜對的比例 |
| `crates/wasm` | 把引擎包成 WebAssembly |
| `web` | 網頁前端（TypeScript 加 Vite） |
| `testdata/sentences.txt` | 準確率測試用的 2,000 句 |

## 需要的工具

- Rust（含 `wasm32-unknown-unknown` 目標：`rustup target add wasm32-unknown-unknown`）
- Node.js
- git 與 git-lfs（下載上游資料用）

## 第一次建置

下載上游資料（固定在特定版本），再轉成資料檔 `data/mofoim.dat`：

```bash
bash scripts/fetch-data.sh
```

```bash
cargo run --release -p mofoim-data-builder -- model data/upstream/dict/chewing_v4 data/mofoim.dat
```

## 執行網頁版

```bash
npm --prefix web install
```

```bash
npm --prefix web run dev
```

打開終端機顯示的網址，點一下打字區就可以開始打。請先把系統輸入法切到英文。

## 測試

引擎與工具的測試，不需要資料檔：

```bash
cargo test --workspace
```

準確率測試，需要先建置資料檔：

```bash
cargo test --release -p mofoim-accuracy -- --ignored
```

網頁前端的測試與型別檢查：

```bash
npm --prefix web test
```

```bash
npm --prefix web run typecheck
```

印出準確率報告（可以用 `--penalty`、`--max-typos`、`--typo-rate` 試不同的參數）：

```bash
cargo run --release -p mofoim-accuracy
```

## 目前的準確率

用 2,000 句測試句子量測；「加入誤觸」是每個按鍵有 3% 的機率換成相鄰鍵。

| 容錯 | 打字方式 | 字 | 整句 |
|---|---|---|---|
| 關閉 | 完全打對 | 96.87% | 82.85% |
| 關閉 | 加入誤觸 | 76.51% | 44.35% |
| 開啟 | 完全打對 | 96.87% | 82.75% |
| 開啟 | 加入誤觸 | 95.27% | 76.25% |

「完全打對」這一列是容錯的底線：容錯不能把本來打對的字改錯。

## 資料來源與授權

程式碼以 MIT 授權釋出，見 [LICENSE](LICENSE)。詞庫、語言模型與測試句子來自其他專案，各自沿用原本的授權，
不包含在這個 repo 裡（測試句子除外），由 `scripts/fetch-data.sh` 下載：

- 詞庫與語言模型：[libchewing-data](https://codeberg.org/chewing/libchewing-data) 的 `dict/chewing_v4`，
  上游標示為 CC BY 4.0。建置出來的 `data/mofoim.dat` 是它的衍生資料，散布時要註明出處。
- 測試句子：[MozTW cc0-sentences](https://github.com/moztw/cc0-sentences)，CC0。
  `testdata/sentences.txt` 是從中抽出的 2,000 句。
