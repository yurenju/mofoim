# 現有輸入法的「選字與訂正」UI 調查

調查日期：2026-11-14。目的：做 mofoim 編輯區訂正 UI 的設計參考。用語照 `GLOSSARY.md`：按鍵序列、編輯區、候選字、釘住、送出。

原始碼的連結都固定在調查當時的 commit：

| 專案 | commit |
|---|---|
| libchewing | `3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c` |
| McBopomofo | `1cb01763f09dd72290021b7d880e668c59d02f82` |
| vChewing-macOS | `149f98c200bf53a28cee1efe826304bca6791385` |
| librime | `bb56876890ba60f446fa2640c244b1794b965eda` |
| fcitx5-chinese-addons | `e8d930f402e359cfd76a101ee3e46815d1331d83` |
| libime | `53df6a248793f9a00c1769e658630de313f3e419` |
| ibus-libpinyin | `42788824ef59ffc03c6f345e20f08ee860dd5c15` |
| mozc | `a4541df29db443b49f81cf54665873b68fde9efa` |

標成 **【未驗證】** 的內容，表示找不到 primary source，只靠記憶或二手描述。

---

## 摘要

### 問題 1：游標移回已組好的字時，字會不會變回讀音？有沒有上下兩條？

- **注音輸入法（新酷音、小麥、威注音、macOS 注音、微軟新注音）不會。** 游標只是在字與字之間移動，已組好的字照樣顯示漢字。正在打、還沒組成字的那幾個注音符號，會插在編輯區的游標位置。引擎內部其實一直留著每個字的讀音：新酷音存的是音節序列，小麥存的是 reading grid。所以「拆字」做得到，只是不會自動發生。
  - 想拆字得用專門的操作。macOS 注音和威注音有 **Shift+Delete**：把游標前那個字退回讀音，並刪掉聲調，剩下的注音放回注音槽讓使用者重打（見 [Apple](#apple-macos-注音)、[vChewing](#vchewing-威注音)）。小麥有一個偏好設定：在注音槽是空的時候按聲調鍵，會直接改掉游標前那個字的聲調。
- **拼音輸入法大多把讀音和字分開顯示，常見的就是「上下兩條」。**
  - fcitx5 pinyin 有兩個顯示區。一個是應用程式內的 client preedit，可以設成「拼音」或「送出預覽（漢字）」。另一個是候選窗上方的 preedit，一律顯示「已選定的漢字＋還沒選的拼音」，游標也畫在這裡。
  - ibus-libpinyin 的 preedit 顯示猜出來的整句漢字，加底線；auxiliary text（候選窗上方）顯示拼音串。
  - RIME 的 preedit 是「已選定的段顯示漢字＋目前這段顯示拼音（照 `preedit_format` 轉換）＋游標後面的原始輸入」。另外 API 提供 `commit_text_preview`，前端可以拿它做第二條。
- **日文 IME（Mozc）分兩個狀態。** 「組字中」（composition）顯示假名讀音。按 Space 進入「轉換中」（conversion）才顯示漢字，並切成幾個文節。轉換中按 Backspace 是取消轉換、整段退回讀音，不是刪字。

### 問題 2：怎麼刪除、重打或修改中間某個字的讀音？Backspace 刪整個字，還是退回讀音？

- **注音系（新酷音、小麥、威注音）：注音槽是空的時候，Backspace 刪掉游標前的整個字（一個音節），不會退回讀音。** 注音槽裡還有沒打完的符號時，Backspace 先一個一個刪符號。
- 要修改中間的字，標準做法是：把游標移到那個字後面，按 Backspace 刪掉，重打注音；新的字會插在游標位置。也可以不刪讀音，直接在那個位置叫出候選字換字（Down 或 Space）。
- 想保留讀音、只改一部分，得靠特殊操作，例如 macOS 和威注音的 Shift+Delete，或小麥的「改聲調」。
- **RIME：Backspace 先「撤銷上一次選字」。** `BackToPreviousInput` 會先重新打開上一個已選定的段（reopen），讓它退回拼音、可以重選；沒有可撤銷的選字時，才刪掉游標前一個輸入字元。Ctrl+Backspace 是刪掉一整個音節。
- **fcitx5 pinyin：** 預設設定 `BackSpaceToUnselect=true`。只要還有已選定的部分，Backspace 就先取消最後一次選字；沒有的話才刪游標前一個拼音字母。Ctrl+Backspace 刪到前一個音節的邊界。
- **Mozc：** 組字中按 Backspace 刪一個假名；轉換中按 Backspace 是 `Cancel`，退回讀音。

### 問題 3：游標怎麼移動？候選字對應游標前面還是後面？

- **注音系的游標以「字（音節）」為單位**，不是以詞為單位，也不會停在一個字的注音符號中間。
- **候選字對應游標前還是游標後，是有名的習慣差異，各家都做成可以設定。**
  - 新酷音：`phrase_choice_rearward`，預設 false，也就是從游標所在的字「往後」選詞。
  - 小麥：`selectPhraseAfterCursorAsCandidate`，預設 false，也就是選「游標前」的字。
  - 威注音：`useRearCursorMode`。原始碼註解把 false 叫做「macOS 注音選字」，把 true 叫做「微軟新注音選字」。
  - 游標在編輯區最尾端時，各家都改成對應最後一個字。
- **一個位置的候選字通常包含「涵蓋這個位置、各種長度的詞」。**
  - 小麥的 `candidatesAt` 把跨過這個位置的所有詞依長度排序，長的在前。
  - 新酷音把候選字依長度分頁：在候選窗裡再按 Down 或 Space，會在「這個位置起算的不同長度」之間輪替。候選窗裡還能用 `j`/`k` 把選字位置左右平移。
- **選字之後游標去哪裡也可以設定。**
  - 新酷音：`auto_shift_cursor`。
  - 小麥：`moveCursorAfterSelectingCandidate`。
  - 威注音：`cursorPlacementAfterSelectingCandidate` 有 0、1、2 三種值：游標不動、往前推一格、回到叫出候選窗之前的位置。
- **拼音系的游標以「拼音字母」為單位**，Ctrl+方向鍵以音節為單位。候選字只對應游標前面的輸入：RIME 只轉換到游標為止，fcitx5 用的是游標前的拼音 `pyBeforeCursor`。所以使用者是靠移動游標，決定「這次要選多長」。
- **Mozc 轉換中，Left/Right 在文節之間移動焦點**，候選窗對應目前的焦點文節。

---

## 比較表

| 輸入法 | 游標移動單位 | Backspace（沒有未完成的讀音時） | 是否顯示讀音 | 候選字對應位置 | 調整分段 | 已送出文字能否 reconversion |
|---|---|---|---|---|---|---|
| 新酷音 libchewing | 音節（字） | 刪游標前的整個字 | 只顯示漢字；未完成的注音另外放在 bopomofo buffer，前端插在游標處 | 可設定：游標起往後（預設）／往前 | 有：Tab 在游標處切換 break/glue；Shift+方向鍵標記範圍後按 Enter 加詞 | 沒有（引擎層） |
| 小麥 McBopomofo | 音節（字） | 刪游標前的整個字 | 只顯示漢字；注音插在游標處；Shift+方向鍵標記時，tooltip 會提示加詞 | 可設定：游標前（預設）／游標後 | 選長詞即可覆蓋分段；Shift+方向鍵標記後加詞 | 沒有 |
| 威注音 vChewing | 音節；Option 可跳到詞的邊界 | 刪一個字；Option+Backspace 刪到詞的邊界；**Shift+Backspace 把前一個字退回讀音** | 只顯示漢字 | 可設定（macOS 式／微軟式） | 有（另有「狂拼」模式會自動重新分段） | 沒有 |
| macOS 注音 | 字 【未驗證】 | Shift+Delete「移除最後一次輸入」，例如刪掉最後一個聲調，等於拆字 | 只顯示漢字 | 游標前 【未驗證，根據威注音原始碼註解】 | 【未驗證】 | 系統文字服務 【未驗證】 |
| 微軟新注音 | 字 【未驗證】 | 【未驗證】 | 只顯示漢字 | 游標後 【未驗證，根據威注音和小麥原始碼註解】 | 【未驗證】 | TSF 有 reconversion 介面；新注音有沒有實作【未驗證】 |
| RIME | 字元；Ctrl+方向鍵以音節為單位；Left 在多段之間會跳段 | 先撤銷上一次選字（reopen），沒有才刪一個字元；Ctrl+Backspace 刪一個音節 | 已選的段顯示漢字，目前這段顯示拼音（`preedit_format`） | 游標前（只轉換到游標為止） | 隱含：移動游標＝決定這段多長；選了部分長度後，剩下的接著轉換 | 沒有 |
| fcitx5 pinyin | 字元；Ctrl+方向鍵以音節為單位 | 先取消上一次選字（`BackSpaceToUnselect` 預設開），沒有才刪一個字元 | 兩條：client preedit（拼音或漢字預覽）＋面板 preedit（已選漢字＋拼音） | 游標前 | 隱含（同 RIME） | 沒有 |
| ibus-libpinyin | 字元；Ctrl 以詞為單位 | 刪一個拼音字元 | 兩條：preedit 顯示漢字整句（加底線），aux 顯示拼音 | 游標處（lookup cursor） | 隱含 | 沒有 |
| Mozc | 組字中以假名為單位；轉換中以文節為單位 | 組字中刪一個假名；轉換中取消轉換、退回讀音 | 組字中顯示假名，轉換中顯示漢字 | 焦點文節 | **Shift+Left/Right 縮短／延長文節** | **有**：Henkan 鍵 Reconvert（ConvertReverse）；Ctrl+Backspace Undo 可撤回剛送出的文字 |

---

## 各輸入法細節

### 新酷音 libchewing（Rust 版）

**資料結構。** `CompositionEditor` 由三樣東西組成：一個游標、一串 `Symbol`（音節或符號），以及每兩個 symbol 之間的 `Gap`（Normal、Break、Glue）。另外還存著使用者的 `Selection`，也就是 mofoim 所說的釘住。註解寫明「游標永遠指在 symbol 之間的縫」。
來源：[composition_editor.rs#L13-L97](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/composition_editor.rs#L13-L97)

新酷音和 mofoim 一樣，存的是讀音、每次重新轉換成字；不同的是它存的是「已經拼好的音節」，不是原始按鍵（ADR-0003 也提到這點）。

**Backspace。**
- 在 `Entering` 狀態（注音槽是空的）：呼叫 `remove_before_cursor()`，刪掉游標前的整個音節。
  來源：[mod.rs#L1374-L1381](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1374-L1381)、[composition_editor.rs#L123-L129](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/composition_editor.rs#L123-L129)
- 在 `EnteringSyllable` 狀態（正在打注音）：`syl.remove_last()`，刪最後一個注音符號。
  來源：[mod.rs#L1642-L1650](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1642-L1650)
- 沒有「把已轉換的字退回注音」的操作。

**插入與刪除會清掉跨過該位置的釘住。** `Composition::insert` 在中間插入音節時，會移除所有跨過插入點的 selection。
來源：[conversion/mod.rs#L276-L301](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/conversion/mod.rs#L276-L301)
新的選字也會清掉和它重疊的舊選字：[conversion/mod.rs#L303-L318](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/conversion/mod.rs#L303-L318)

**游標與選字。**
- Left/Right 一次移一個 symbol，可以設定是否循環（`auto_wrap_cursor`）。
  來源：[mod.rs#L1336-L1353](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1336-L1353)
- Down 進入選字。
  來源：[mod.rs#L1469-L1472](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1469-L1472)
- 選字位置由 `symbol_for_select` 決定；游標在最尾端時改用最後一個字。
  來源：[composition_editor.rs#L182-L189](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/composition_editor.rs#L182-L189)
- `PhraseSelector::init`：
  - forward 模式：從游標所在的字往後，延伸到下一個 break。
  - rearward 模式：從前一個 break 開始，到游標所在的字為止。
  - 接著把範圍縮短，直到詞庫查得到為止。
  - 在候選窗裡再按 Down 或 Space，`next()` 會換下一種長度。
  來源：[selection/phrase.rs#L44-L76](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/selection/phrase.rs#L44-L76)、[#L183-L206](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/selection/phrase.rs#L183-L206)、[mod.rs#L1890-L1905](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1890-L1905)
- 候選窗裡按 `j`/`k`：把選字的起點左移或右移一個字，重建候選字。
  來源：[mod.rs#L1906-L1959](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1906-L1959)
- 候選窗裡按 Backspace 或 Up：取消選字。
  來源：[mod.rs#L1877-L1889](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1877-L1889)
- 選定之後，`auto_shift_cursor` 決定游標要不要跳到詞的後面。
  來源：[mod.rs#L1800-L1830](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1800-L1830)
- 相關選項的預設值：[mod.rs#L77-L124](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L77-L124)

**分段調整（Tab）。**
- 游標在詞的邊界時，按 Tab 插入 glue（把兩邊黏成一個詞）。
- 游標不在詞的邊界時，按 Tab 插入 break（強制在這裡斷開）。
- 游標在最尾端時，按 Tab 輪替整句的第 n 好轉換結果（`nth_conversion`）。

來源：[mod.rs#L1417-L1429](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1417-L1429)

**Shift+Left/Right。** 進入 `Highlighting` 狀態，標記一個範圍，按 Enter 就把這個範圍加進使用者詞庫。
來源：[mod.rs#L1447-L1460](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1447-L1460)、[#L2126-L2156](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L2126-L2156)

**學習。** 送出時，`auto_learn_user_history`（預設開）會把這次送出的詞記進使用者詞庫。
來源：[mod.rs#L1001-L1007](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L1001-L1007)

另外有 `auto_snapshot_selections`（預設關）：開啟後，引擎會把目前猜出來的詞自動變成 selection，等於自動釘住，之後打字就不會再改到前面的詞。
來源：[mod.rs#L857-L877](https://github.com/chewing/libchewing/blob/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c/src/editor/mod.rs#L857-L877)

### 小麥注音 McBopomofo

**資料結構。** `ReadingGrid` 存的是每個字的讀音，上面再建出各種長度詞的 node；每打一個字就重新 walk 一次，找出最佳路徑。顯示時把整句漢字切成游標前、游標後兩半，正在打的注音插在中間。
來源：[KeyHandler.mm#L2375-L2420](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/KeyHandler.mm#L2375-L2420)

**Backspace 的順序：**
1. 注音槽裡只有聲調，就清掉聲調。
2. 注音槽是空的，就 `deleteReadingBeforeCursor()` 刪掉游標前的整個讀音，然後重新 walk。
3. 其他情況，刪掉一個注音符號。

來源：[KeyHandler.mm#L1095-L1124](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/KeyHandler.mm#L1095-L1124)

**修改已組好的字的讀音。** `allowChangingPriorTone`（Issue 753）：注音槽是空的時候按聲調鍵，會把游標前那個字的讀音換成新聲調。
來源：[KeyHandler.mm#L484-L508](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/KeyHandler.mm#L484-L508)

**讀音不存在時。** 拼出的讀音在詞庫裡查不到，會發出錯誤提示；`keepReadingUponCompositionError` 決定要保留注音讓使用者修改，還是直接清掉。
來源：[KeyHandler.mm#L512-L538](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/KeyHandler.mm#L512-L538)

**游標與 Shift+方向鍵。** Left 移一個讀音。注音槽不是空的時候，按方向鍵只會發出錯誤提示，不移動游標。Shift+Left 進入 Marking 狀態，用來加詞。
來源：[KeyHandler.mm#L945-L987](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/KeyHandler.mm#L945-L987)、Marking：[#L1316-L1340](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/KeyHandler.mm#L1316-L1340)

**選字對應的位置。** `computeActualCursorIndex`：游標在最尾端時用 cursor−1。`selectPhraseAfterCursorAsCandidate` 是 false（預設）時也用 cursor−1，也就是游標前的字。
來源：[KeyHandler.mm#L2545-L2569](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/KeyHandler.mm#L2545-L2569)

`candidatesAt` 列出所有跨過這個位置的 node，依長度排序，長的在前。
來源：[reading_grid.cpp#L210-L236](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/Engine/gramambular2/reading_grid.cpp#L210-L236)

官方手冊說明：游標在兩字之間時，要選前面還是後面的字「交給用戶選擇」；Tab／Shift+Tab 可以不開候選窗，直接輪替候選字。
來源：[使用手冊](https://github.com/openvanilla/McBopomofo/wiki/使用手冊)

**選字後會不會重算其他位置。** 會。`overrideCandidate` 會把選中的 node 固定成高分，並 reset 所有和它重疊的其他 node，然後重新 walk 整句。所以選了一個長詞之後，相鄰位置的猜測也可能跟著改。
來源：[KeyHandler.mm#L171-L204](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/KeyHandler.mm#L171-L204)、[reading_grid.cpp#L371-L413](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/Engine/gramambular2/reading_grid.cpp#L371-L413)

**選字記憶。**
- 選字時呼叫 `UserOverrideModel::observe` 記錄下來。
- 之後打字時，用 `suggest` 自動套用；套用的方式是 override，但分數不一定最高。
  來源：[KeyHandler.mm#L540-L552](https://github.com/openvanilla/McBopomofo/blob/1cb01763f09dd72290021b7d880e668c59d02f82/Source/KeyHandler.mm#L540-L552)
- 手冊說這份記憶過一段時間會「漸漸忘掉」。

### vChewing 威注音

**Shift+Backspace 拆字。** 原始碼註解寫著「引入 macOS 內建注音輸入法的行為，允許用 Shift+BackSpace 解構前一個漢字的讀音」。做法是從組字器刪掉前一個字，再把它的讀音（去掉聲調）逐個符號放回注音槽。偏好設定 `specifyShiftBackSpaceKeyBehavior` 可以改成「清空整段」。
來源：[InputHandler_HandleStates.swift#L1250-L1279](https://github.com/vChewing/vChewing-macOS/blob/149f98c200bf53a28cee1efe826304bca6791385/Packages/vChewing_OSNeutral_LibVanguard/Sources/LibVanguard/InputHandler/InputHandler_HandleStates.swift#L1250-L1279)

**Option+Backspace。** 一次刪到最近的詞邊界（`getStepsToNearbyNodeBorder`）。
來源：[同檔 #L1281-L1310](https://github.com/vChewing/vChewing-macOS/blob/149f98c200bf53a28cee1efe826304bca6791385/Packages/vChewing_OSNeutral_LibVanguard/Sources/LibVanguard/InputHandler/InputHandler_HandleStates.swift#L1281-L1310)

**選字游標的兩種模式。** `useRearCursorMode`：false 是「macOS 注音選字」，true 是「微軟新注音選字」。
來源：[InputHandler_CoreProtocol.swift#L703-L715](https://github.com/vChewing/vChewing-macOS/blob/149f98c200bf53a28cee1efe826304bca6791385/Packages/vChewing_OSNeutral_LibVanguard/Sources/LibVanguard/InputHandler/InputHandler_CoreProtocol.swift#L703-L715)

選字後游標放在哪裡（0、1、2）：[#L224-L236](https://github.com/vChewing/vChewing-macOS/blob/149f98c200bf53a28cee1efe826304bca6791385/Packages/vChewing_OSNeutral_LibVanguard/Sources/LibVanguard/InputHandler/InputHandler_CoreProtocol.swift#L224-L236)

威注音還有一個「狂拼」模式（`InputHandler_FuriousResegmentation.swift`），會對尾端的按鍵做替代切分、重新分段。這和 mofoim「不事先切字」的想法接近，值得之後細看。本次沒有深入。

### Apple macOS 注音

- 官方說明：音節加上聲調後會自動轉換，之後的字會依辭典回頭調整前面的字；按 Space 叫出候選字。官方範例：打出「植物學」後按 **Shift+Delete「可移除最後一次輸入」**，結果是刪掉「ˊ」聲調。換句話說，最後一個字會退回讀音（ㄒㄩㄝ），等使用者重打聲調。
  來源：[Apple 支援：使用繁體注音輸入](https://support.apple.com/zh-hk/guide/chinese-input-method/zhuyin-traditional-cimzt15531/104/mac/15.0)
- 鍵盤快速鍵表把 Shift-Delete 寫成「Undo the last substitution」，不同版本的文件說法不一致。
  來源：[Keyboard shortcuts for the Traditional Chinese input method](https://support.apple.com/guide/chinese-input-method/cimkeyt36646/mac)
- 官方文件沒有說明游標移動，也沒有說明選字對應哪一邊。「游標前」只來自威注音原始碼的註解。**【未驗證】**

### 微軟新注音（Microsoft Bopomofo）

- 官方文件寫明：
  - 打完注音後按 Down 開啟候選窗。
  - 還沒進入候選窗時按 Up，可以直接換成別的候選字，不必打開候選窗。
  - Tab 或 Right 把候選窗展開成表格；Esc 關閉候選窗。
  - 官方範例：「夏天」的第一個字先被猜錯，打完第二個字後，第一個字被回頭改正。
  - 設定裡有「IME learning options」。

  來源：[Microsoft Traditional Chinese IME（支援頁）](https://support.microsoft.com/en-us/windows/microsoft-traditional-chinese-ime-ef596ca5-aff7-4272-b34b-0ac7c2631a38)、[Microsoft Learn: Traditional Chinese IME](https://learn.microsoft.com/globalization/input/traditional-chinese-ime)
- 官方文件**沒有**描述游標、Backspace、選字對應的位置，也沒有描述重新轉換。「選字對應游標後的字」只來自小麥與威注音原始碼的註解。**【未驗證】**

### 微軟拼音

官方快捷鍵表寫明：Left/Right 是「在輸入區移動插入點」；Enter 關閉候選窗、保留已輸入的內容；Esc 關閉候選窗並刪掉已輸入的內容。另有 Fuzzy Pinyin（模糊音）選項。
來源：[Microsoft Simplified Chinese IME](https://support.microsoft.com/en-us/windows/microsoft-simplified-chinese-ime-9b962a3b-2fa4-4f37-811c-b1886320dd72)

游標單位、Backspace、重新轉換都沒有文件。**【未驗證】**

### RIME / librime

**Preedit 組成。** `Composition::GetPreedit`：
- 已確認的段顯示候選字的文字（漢字）。
- 最後一段（highlighted）顯示 `cand->preedit()`，也就是音節化之後的拼音，再經 `preedit_format` 轉換。
- 游標後面的原始輸入照原樣接在最後。
- soft cursor（`caret`）插在游標處。

來源：[composition.cc#L24-L96](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/composition.cc#L24-L96)、[script_translator.cc#L567-L583](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/script_translator.cc#L567-L583)、[translator_commons.cc#L133-L134](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/translator_commons.cc#L133-L134)

API 另外提供 `commit_text_preview`（如果現在送出，會得到的漢字）：[rime_api.h#L159](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime_api.h#L159)

**只轉換到游標為止。** `Compose` 只拿 `input.substr(0, caret_pos)` 去分段和轉換；只有游標剛好落在已確認位置時，才往後多轉一段。所以候選字對應的是游標前（或游標剛好在後面那一段）的輸入。
來源：[engine.cc#L231-L245](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/engine.cc#L231-L245)、[#L266-L270](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/engine.cc#L266-L270)

選字之後，如果這段已經延伸到游標，游標就跳到最尾端；如果還沒到，就繼續轉換剩下的部分。
來源：[engine.cc#L340-L363](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/engine.cc#L340-L363)

**Backspace 與撤銷選字。**
- fluid editor：Backspace 是 `BackToPreviousInput`，依序嘗試 `ReopenPreviousSegment`、`ReopenPreviousSelection`、`PopInput`，也就是先撤銷選字，沒有可撤銷的才刪字元。
- express editor：Backspace 是 `RevertLastEdit`。
- 兩種 editor 都把 Ctrl+Backspace 設成 `BackToPreviousSyllable`，以音節為單位刪除。

來源：[editor.cc#L125-L160](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/editor.cc#L125-L160)、[#L187-L217](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/editor.cc#L187-L217)、[context.cc#L204-L245](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/context.cc#L204-L245)

`Segment::Reopen`：游標剛好在這段的結尾時，保留原本的候選和選擇（狀態變成 kGuess）；否則整段作廢（kVoid）。
來源：[segmentation.cc#L26-L43](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/segmentation.cc#L26-L43)

**游標移動。**
- Left 是 `Rewind`：有多個還沒確認的段、而且游標在段的邊界時，往左跳一整段；否則移一個字元。
- Ctrl+Left 以音節為單位移動。
- `syllable_jump_position` 可以設定跳到分隔符號的前面或後面。

來源：[navigator.cc#L41-L63](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/navigator.cc#L41-L63)、[#L130-L155](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/navigator.cc#L130-L155)

### fcitx5-chinese-addons pinyin（libime）

**兩個 preedit。** `PinyinEngine::preedit` 回傳兩個 Text：
- client preedit（顯示在應用程式裡）：依 `PreeditMode` 決定顯示什麼，選項有不顯示、`ComposingPinyin`（預設）、`CommitPreview`（底線漢字）。
- 面板 preedit：一律顯示 `preeditWithCursor`，也就是「已選漢字＋拼音」，並帶游標。

來源：[pinyin.cpp#L418-L461](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L418-L461)、[pinyin.h#L105-L108](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.h#L105-L108)、[#L203-L211](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.h#L203-L211)、[libime preeditWithCursor](https://github.com/fcitx/libime/blob/53df6a248793f9a00c1769e658630de313f3e419/src/libime/pinyin/pinyincontext.cpp#L889-L935)

這裡有一個值得 mofoim 參考的細節。程式註解說明：顯示拼音時刻意不套用模糊音，避免把 "qign" 顯示成 "qing"，否則使用者會搞不清自己實際打了什麼。

**按鍵行為。**
- Backspace：`BackSpaceToUnselect` 開啟（預設）而且還有已選定的部分時，取消上一次選字；否則刪一個字元。
- Ctrl+Backspace：刪到前一個音節的邊界。
- Left：游標在已選定區的邊界時，先取消選字再移動。
- Ctrl+Left/Right：以音節為單位移動。

來源：[pinyin.cpp#L2245-L2310](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L2245-L2310)、[pinyin.h#L312-L314](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.h#L312-L314)

`setCursor` 會呼叫 `cancelTill(pos)`：游標移進已選定區時，自動撤銷那些選字。`cancel()` 會清掉 lattice 重算。
來源：[pinyincontext.cpp#L510-L521](https://github.com/fcitx/libime/blob/53df6a248793f9a00c1769e658630de313f3e419/src/libime/pinyin/pinyincontext.cpp#L510-L521)、[#L627-L648](https://github.com/fcitx/libime/blob/53df6a248793f9a00c1769e658630de313f3e419/src/libime/pinyin/pinyincontext.cpp#L627-L648)

候選字取自游標前的拼音：[pinyin.cpp#L566-L576](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L566-L576)

### ibus-libpinyin

- preedit 是「猜出來的整句＋還沒解析的拼音」，加單底線。
  來源：[PYPPinyinEditor.cc#L245-L292](https://github.com/libpinyin/ibus-libpinyin/blob/42788824ef59ffc03c6f345e20f08ee860dd5c15/src/PYPPinyinEditor.cc#L245-L292)
- auxiliary text 是拼音串。
  來源：[#L298-L330](https://github.com/libpinyin/ibus-libpinyin/blob/42788824ef59ffc03c6f345e20f08ee860dd5c15/src/PYPPinyinEditor.cc#L298-L330)
- Backspace 刪一個拼音字元，Ctrl+Backspace 刪一個詞；Left 移一個字元，Ctrl+Left 移一個詞。
  來源：[PYPPhoneticEditor.cc#L107-L118](https://github.com/libpinyin/ibus-libpinyin/blob/42788824ef59ffc03c6f345e20f08ee860dd5c15/src/PYPPhoneticEditor.cc#L107-L118)、[#L165-L176](https://github.com/libpinyin/ibus-libpinyin/blob/42788824ef59ffc03c6f345e20f08ee860dd5c15/src/PYPPhoneticEditor.cc#L165-L176)、[#L563-L577](https://github.com/libpinyin/ibus-libpinyin/blob/42788824ef59ffc03c6f345e20f08ee860dd5c15/src/PYPPhoneticEditor.cc#L563-L577)

### Mozc（日文，作為分段調整與 reconversion 的參考）

MS-IME 相容 keymap 的關鍵幾行（依 `ms-ime.tsv` 行號）：

| 狀態 | 按鍵 | 動作 | 行號 |
|---|---|---|---|
| Composition | Backspace | Backspace | L3 |
| Composition | Left | MoveCursorLeft | L48 |
| Composition | Space | Convert | L59 |
| Conversion | Backspace | Cancel（退回讀音） | L64 |
| Conversion | Left / Right | SegmentFocusLeft / Right | L110 / L116 |
| Conversion | Shift+Left / Shift+Right | SegmentWidthShrink / Expand | L121 / L123 |
| Precomposition | Ctrl+Backspace | Undo | L143 |
| Precomposition / DirectInput | Henkan | Reconvert | L148 / L136 |

來源：[ms-ime.tsv](https://github.com/google/mozc/blob/a4541df29db443b49f81cf54665873b68fde9efa/src/data/keymap/ms-ime.tsv)

- **Undo：** 先 `converter->Revert()` 撤回學習記錄，再透過 `DeletionRange` 要求應用程式刪掉剛送出的文字，然後恢復送出前的狀態。需要應用程式支援 `DELETE_PRECEDING_TEXT`。
  來源：[session.cc#L1287-L1325](https://github.com/google/mozc/blob/a4541df29db443b49f81cf54665873b68fde9efa/src/session/session.cc#L1287-L1325)
- **Reconvert（ConvertReverse）：** 對應用程式裡選取的已送出文字，用 `GetReadingText` 反查讀音，放回 composer，然後直接進入轉換。
  來源：[session.cc#L1202-L1250](https://github.com/google/mozc/blob/a4541df29db443b49f81cf54665873b68fde9efa/src/session/session.cc#L1202-L1250)
- 文節長度調整的實作：[session.cc#L2626](https://github.com/google/mozc/blob/a4541df29db443b49f81cf54665873b68fde9efa/src/session/session.cc#L2626)
- macOS 日文輸入也有 Reverse Conversion（Control-Shift-R，或按兩次 Kana 鍵），但要看應用程式支不支援。
  來源：[Apple：Reverse a conversion](https://support.apple.com/lt-lt/guide/japanese-input-method/reverse-a-conversion-jpim10309/6.2.1/mac/10.15)

### 平台 API（只列和訂正 UI 有關的部分）

- **Windows TSF `ITfFnReconversion`：** 由 text service 實作，用 `QueryRange`、`GetReconversion`、`Reconvert` 讓使用者替已插入的文字換成其他候選。
  來源：[Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/ctffunc/nn-ctffunc-itffnreconversion)
- **macOS `NSTextInputClient setMarkedText:selectedRange:replacementRange:`：** marked text 可以帶 attributed string 來畫分段底線。`replacementRange` 可以替換文件裡已經存在的文字，這是 reconversion 的基礎。
  來源：[Apple Developer](https://developer.apple.com/documentation/appkit/nstextinputclient/setmarkedtext(_:selectedrange:replacementrange:))
  這兩個參數的細部語意，因為頁面抓不到內文，是依記憶描述。**【未驗證】**
- **Web EditContext `textformatupdate`：** IME 用 `TextFormat` 的 `rangeStart`、`rangeEnd`、`underlineStyle`、`underlineThickness` 告訴頁面每段該畫哪種底線，屬於實驗性 API。mofoim 的 web 前端自己就是 IME、自己畫編輯區，所以這個 API 主要是參考它的底線樣式語意：OS IME 慣用「目前焦點段畫粗底線，其他段畫細底線」。這個慣例本身**【未驗證】**。
  來源：[MDN TextFormatUpdateEvent](https://developer.mozilla.org/en-US/docs/Web/API/TextFormatUpdateEvent)

---

## 對 mofoim 的啟示

### 先釐清 mofoim 和其他輸入法的根本差異

1. **其他注音輸入法的編輯區單位是「已經確定的音節」。** 字的邊界早就切死了，所以「游標在第幾個字」、「刪掉這個字」都很好定義。mofoim 的按鍵序列不事先切字（ADR-0003），字的邊界是引擎猜出來的，按一個鍵就可能改變。所以「游標在哪個字後面」需要另外定義：可以定在按鍵序列上，也可以定在目前的猜測上。
2. **mofoim 的錯誤來源和其他輸入法不同。** 其他輸入法要修的是「選錯同音字」，注音本身是使用者打的，假設是對的。mofoim 還要修「引擎把誤觸解讀錯」，也就是某個字的讀音本身猜錯了。所以只換同音候選字不夠，訂正 UI 必須能**換讀音**。這也是 macOS 的 Shift+Delete 與小麥的改聲調值得參考的原因：它們是現有輸入法中少數能動到讀音的操作。

### 可以考慮的 UI 選項

**A. 游標以「字」為單位，按鍵序列跟著對應（新酷音和小麥模式）**
- 引擎回傳目前猜測中每個字對應的按鍵範圍。游標停在字和字之間，前端把它換算成按鍵序列裡的位置。
- 優點：使用者最熟悉，所有注音輸入法都這樣做。
- 取捨：
  - 游標前後的字在下一次重新猜測時可能被重新切分，游標的位置需要定義成在重新猜測後仍然有效，例如綁在按鍵序列的 index 上。
  - ADR-0003 寫著「第一版沒有游標」，加入游標要另寫 ADR。

**B. 「上下兩條」：上面顯示讀音，下面顯示猜出來的字（fcitx5、ibus-libpinyin、RIME 模式）**
- 上面那條可以顯示引擎採用的注音解讀，或顯示原始按鍵對應的注音，下面那條顯示漢字。
- 這對 mofoim 特別有價值：容錯會讓「使用者打的」和「引擎認為使用者想打的」不同，兩條並列，使用者才看得出哪裡被改了。
- fcitx5 的經驗值得照搬：讀音那條顯示「實際打了什麼」，不要被模糊音或容錯「美化」。可以考慮在被容錯改過的符號上加標記，例如用不同顏色。
- 取捨：佔用更多版面；手機上 fcitx5 預設關掉 client preedit。

**C. 拆字操作：把游標前的字退回讀音（macOS 注音和威注音的 Shift+Backspace）**
- 在 mofoim 裡可以這樣做：把那個字對應的按鍵從「已猜測」狀態移出來，顯示成可編輯的注音符號，讓使用者改其中一個符號；或者乾脆刪掉那個字的按鍵、重打。
- 最小可行版本是「移到某個字後面，按 Backspace 刪掉那個字的所有按鍵，再重打」。這就是新酷音和小麥的 Backspace 語意，但要先有 A 的游標。

**D. Backspace 先撤銷選字（RIME 和 fcitx5 模式）**
- 如果最後一個動作是釘住，Backspace 先解除釘住，而不是刪按鍵。
- 對使用者來說這就是 undo 的一種，代價很低，和「按鍵序列是唯一事實來源」也不衝突，因為釘住是另外疊在按鍵序列上的一層。
- 取捨：使用者可能預期 Backspace 一定會刪東西，所以 RIME 和 fcitx5 都把這做成可以設定。

**E. 候選字對應游標前還是游標後**
- 各家都做成可以設定。建議先固定一種，例如和小麥預設一樣選「游標前」，配合「選字後游標不動」。
- 候選列表應該包含「跨過這個位置、各種長度的詞」（小麥的 `candidatesAt`）。
- mofoim 的候選字還可以多一層：**同一個位置、不同讀音解讀的候選**，例如誤觸的另一種解讀。可以用分組或標記跟同音候選區分開來。

**F. 釘住後要不要重算其他位置**
- 小麥選字後會重新 walk 整句，新酷音的 selection 是限制條件、其他位置照樣重算。mofoim 的「釘住」定義也是這樣，方向一致。
- 要注意的副作用：在中間插入或刪除按鍵時，新酷音會清掉跨過插入點的 selection。mofoim 也需要定義「釘住範圍內的按鍵被改了」時怎麼處理，例如自動解除釘住。

**G. 分段調整**
- 有兩種做法：
  - 新酷音的 Tab，在游標處切換 break 和 glue。
  - Mozc 的 Shift+Left/Right，縮短或延長焦點文節。
- 對 mofoim 來說，強迫「這裡斷開」或「這裡黏起來」，可以當作一種限制條件，和釘住並列，直接減少引擎要嘗試的切法。
- 取捨：注音輸入法使用者很少用到這類操作，可以晚點做。

**H. 學習與 reconversion**
- 選字記憶：小麥的 user override 會隨時間淡忘，新酷音在送出時學習。這些屬於引擎層的功能，可以之後再做。
- Reconversion（Mozc 的 Reconvert、TSF、macOS 的 replacementRange）要能從已送出的文字反查按鍵序列，跟「送出後引擎不再更動」的定義衝突。如果要做，比較可行的是 Mozc 式的「Undo 剛送出的文字」：把送出前的按鍵序列留著，撤銷後還原到編輯區。web 前端自己掌握文字區，所以做得到。

**I. 錯字提示樣式**
- 現有輸入法大多只用「底線」和「焦點段反白」：fcitx5 是 `Underline` 加上 `HighLight`，ibus 是單底線。讀音查不到時，用錯誤音效或 tooltip（小麥的 `errorCallback`）。
- 本次沒找到任何主流注音輸入法用顏色標出「這個字可能是容錯修正的結果」，這可能是 mofoim 的差異化空間。**【未找到前例】**

---

## 未驗證或未找到 primary source 的主張

- 微軟新注音的游標單位、Backspace 行為、選字對應「游標後」：官方文件沒有寫，只有小麥和威注音原始碼註解這種二手說法。
- macOS 注音選字對應「游標前」，以及游標的移動單位：同上。
- 微軟拼音的 Backspace 行為與重新轉換：官方文件沒有寫。
- 微軟新注音有沒有實作 TSF reconversion：沒有找到文件。
- `NSTextInputClient` 的 `selectedRange` 與 `replacementRange` 的細部語意：Apple 頁面抓不到內文，依記憶描述。
- 「焦點段粗底線、其他段細底線」是 OS IME 慣例：依記憶描述。
- 搜狗輸入法、Google 拼音：本次沒有調查，沒有找到可引用的官方說明。

## 來源列表

**原始碼**
- libchewing：https://github.com/chewing/libchewing/tree/3ac468e53d9ad8b09429cbf5fd20e18b7402eb0c
- McBopomofo：https://github.com/openvanilla/McBopomofo/tree/1cb01763f09dd72290021b7d880e668c59d02f82
- vChewing-macOS：https://github.com/vChewing/vChewing-macOS/tree/149f98c200bf53a28cee1efe826304bca6791385
- librime：https://github.com/rime/librime/tree/bb56876890ba60f446fa2640c244b1794b965eda
- fcitx5-chinese-addons：https://github.com/fcitx/fcitx5-chinese-addons/tree/e8d930f402e359cfd76a101ee3e46815d1331d83
- libime：https://github.com/fcitx/libime/tree/53df6a248793f9a00c1769e658630de313f3e419
- ibus-libpinyin：https://github.com/libpinyin/ibus-libpinyin/tree/42788824ef59ffc03c6f345e20f08ee860dd5c15
- mozc：https://github.com/google/mozc/tree/a4541df29db443b49f81cf54665873b68fde9efa

**官方文件**
- 小麥注音使用手冊：https://github.com/openvanilla/McBopomofo/wiki/使用手冊
- Apple 繁體注音：https://support.apple.com/zh-hk/guide/chinese-input-method/zhuyin-traditional-cimzt15531/104/mac/15.0
- Apple 繁中快速鍵：https://support.apple.com/guide/chinese-input-method/cimkeyt36646/mac
- Apple 日文 Reverse a conversion：https://support.apple.com/lt-lt/guide/japanese-input-method/reverse-a-conversion-jpim10309/6.2.1/mac/10.15
- Microsoft Traditional Chinese IME：https://support.microsoft.com/en-us/windows/microsoft-traditional-chinese-ime-ef596ca5-aff7-4272-b34b-0ac7c2631a38
- Microsoft Learn Traditional Chinese IME：https://learn.microsoft.com/globalization/input/traditional-chinese-ime
- Microsoft Simplified Chinese IME：https://support.microsoft.com/en-us/windows/microsoft-simplified-chinese-ime-9b962a3b-2fa4-4f37-811c-b1886320dd72

**平台 API**
- TSF ITfFnReconversion：https://learn.microsoft.com/en-us/windows/win32/api/ctffunc/nn-ctffunc-itffnreconversion
- NSTextInputClient setMarkedText：https://developer.apple.com/documentation/appkit/nstextinputclient/setmarkedtext(_:selectedrange:replacementrange:)
- MDN TextFormatUpdateEvent：https://developer.mozilla.org/en-US/docs/Web/API/TextFormatUpdateEvent
