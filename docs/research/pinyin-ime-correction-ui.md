# 拼音輸入法怎麼訂正長句中段或尾端的錯字：UI 調查

調查日期：2026-11-15。目的：替 [issue #2](https://github.com/yurenju/mofoim/issues/2) 找設計參考。issue 的情境是：打了一個長句，最後的「的字」被猜成「得自」，在 mofoim 目前的模型下（[ADR 0004](../adr/0004-movable-cursor-and-pinned-segments.md)）很難只改掉這一段。

這份文件是 [`ime-correction-ui.md`](ime-correction-ui.md) 的補充，那份已經寫過的內容這裡不重複，包括：新酷音、小麥、威注音、macOS 注音、微軟新注音、RIME 核心（`Compose` 只轉換到游標為止、`BackToPreviousInput`、`Segment::Reopen`、Left 的 `Rewind`）、fcitx5 pinyin（兩個編輯區、`BackSpaceToUnselect`、`cancelTill`、`pyBeforeCursor`）、ibus-libpinyin 的基本按鍵，以及 Mozc。這裡只補拼音輸入法特有的細節。

用語照 `GLOSSARY.md`：按鍵序列、編輯區、游標、候選字、整句猜測、釘住、撤銷釘住、送出。引用原文、或原文的識別字（例如 `preedit`、`commit`）時保留原樣。

原始碼的連結都固定在調查當時的 commit：

| 專案 | commit |
|---|---|
| rime/librime | `bb56876890ba60f446fa2640c244b1794b965eda` |
| rime/weasel（小狼毫） | `f96ed24a8a535961b6ffc7160d98851a69cee4f8` |
| rime/squirrel（鼠鬚管） | `b7970a93b05ae1ca1bcd97888fd6aa3cd0458d3e` |
| iDvel/rime-ice（霧凇拼音） | `927e1cf1d3eeb9bf4be85e3f684dba4dd485b6f6` |
| Mintimate/oh-my-rime | `0cf3d63c61d07a84834c031bad4ef8ac39b322d0` |
| fcitx/fcitx5-chinese-addons | `e8d930f402e359cfd76a101ee3e46815d1331d83` |
| fcitx/libime | `53df6a248793f9a00c1769e658630de313f3e419` |
| libpinyin/libpinyin | `85905e59cb19d0343cae84d831027aee113420b8` |
| libpinyin/ibus-libpinyin | `42788824ef59ffc03c6f345e20f08ee860dd5c15` |
| sunpinyin/sunpinyin | `e4be7e64e4a168a684b26679b7e076118a2c97f7` |
| AOSP PinyinIME（Google 拼音 Android 開源版） | `49aebad1c1cfbbcaa9288ffed5161e79e57c3679` |
| osfans/trime（同文） | `2e2b4d5394297745bf42ff6260bb547a271f378a` |
| fcitx5-android | `eb5b50216e70484a009933d9519db76005f1faf3` |

閉源輸入法沒有原始碼，引用的是官方說明頁、官方升級日誌、官方 FAQ 或廠商自己寫的 App Store 描述。標成 **【未驗證】** 的內容，表示找不到 primary source，只有二手來源（部落格、下載站、新聞評測）或記憶，並註明是哪一種。

---

## 摘要

### 先講結論：「的字」被猜成「得自」時，各家怎麼改

把幾家有官方說明的做法對到 issue #2 的情境，大致是這幾條路：

1. **游標退到錯誤前面，先釘住前段，再選後段**（百度 PC、QQ 拼音 PC、RIME、fcitx5、sunpinyin、AOSP 都是這樣）。百度和 QQ 的官方說明都用一模一樣的教學範例：按 ← 把游標退到錯的音節前面，此時候選字只對應游標前的拼音，選第一個把前段釘住，接著候選字換成剩下的拼音，再選對的字。**這就是 mofoim ADR 0004 現在的模型**，只是這些輸入法把整段流程壓得比較短（見下面「可以直接套用的做法」）。
2. **游標停在中間，前段的猜測不動，直接替游標後面那段選字**（搜狗 PC 4.2 的官方升級日誌、ibus-libpinyin）。libpinyin 的限制條件可以加在任意位置，所以「只改最後一個詞、前面不必先釘住」在引擎層做得到。這個方向和 ADR 0004「候選字只對應游標前」正好相反。
3. **游標直接移進已經轉好的漢字裡換字**（舊版微軟拼音 3.0 的官方教學，以及「新體驗」風格【未驗證細節】）。編輯區顯示的是漢字，← 一次移一個字，候選窗跟著游標所在的字打開。
4. **點一下錯的地方**（fcitx5、fcitx5-android、Trime、百度手機、搜狗手機）。fcitx5 的實作是：點到已釘住的漢字範圍，就一直撤銷釘住直到涵蓋點擊位置；點到拼音範圍，就把游標移過去。
5. **送出之後再撤銷重選**（QQ 拼音 Ctrl+Backspace「恢復上次輸入」）。

### UI 模式分類

| 模式 | 做法 | 例子 |
|---|---|---|
| **A. 游標切開句子，由前往後釘住** | ←/→ 在拼音串裡移動游標，候選字只對應游標前；選了之後前段釘住，剩下的接著猜 | 百度 PC、QQ 拼音、微軟拼音（現行）、RIME、fcitx5、sunpinyin、AOSP |
| **B. 候選字對應游標後，前段不動** | 游標停在中間，候選字從游標處開始列，前段維持猜測、不必先釘住 | 搜狗 PC 4.2、ibus-libpinyin（libpinyin 限制條件） |
| **C. 直接在漢字上改** | 編輯區顯示整句漢字，游標以字為單位移動，候選窗對應游標所在的字 | 舊版微軟拼音（3.0、新體驗風格）；谷歌拼音「內嵌編輯模式」【細節未驗證】 |
| **D. 點擊編輯區定位** | 點拼音或漢字，把游標移過去；點到已釘住的漢字就撤銷釘住 | fcitx5 pinyin、fcitx5-android、Trime、百度手機 V3.3/V3.7、搜狗手機「拼音串可編輯」 |
| **E. 以詞定字** | 先選一個詞，再用 `[` `]`（或 `,` `.`、`-` `=`）只取它的首字或尾字 | 谷歌拼音、百度、QQ、搜狗、手心、fcitx5（`ChooseCharFromPhrase`）、rime-ice（lua） |
| **F. 快速定位** | Ctrl+字母、空白+字母、輔助碼，直接跳到某個音節 | 搜狗 PC 5.2、手心輸入法 |
| **G. 選取拼音串／依音節跳** | Shift+←/→ 選取一段拼音；Tab 或 Ctrl+←/→ 一次跳一個音節 | 搜狗 PC 5.0；rime-ice 把 Tab 綁成跳音節；RIME、fcitx5 的 Ctrl+←/→ |
| **H. 整句的其他猜法** | 候選字裡放「整句的第 2、3 好結果」，選了等於一次改掉所有不同的地方 | libpinyin NBEST（`diff_result`）、sunpinyin `OTHER_BEST_TAIL` |
| **I. 倒退鍵先撤銷釘住** | 有釘住的段時，倒退鍵先撤銷最後一次釘住，不刪按鍵 | sunpinyin `m_backspaceCancel`、fcitx5、RIME；AOSP 刪到釘住的字就解開 |
| **J. 送出後撤銷** | 剛送出的文字退回編輯區，拼音串還原，可以重選 | QQ 拼音 Ctrl+Backspace；（日文 Mozc Undo，見前一份文件） |
| **K. 行動裝置手勢** | 空白鍵左右滑動移動游標；九鍵左側音節欄逐音節篩選 | 搜狗手機、百度手機、fcitx5-android |
| **L. 錯誤標記** | 在編輯區標出「這裡可能打錯」 | 搜狗手機「糾錯標記」；訊飛紅色倒 V【未驗證】 |

### 編輯區怎麼顯示

- **「已釘住的漢字＋還沒釘住的拼音」是主流**：sunpinyin、AOSP、fcitx5（`ComposingPinyin`）、RIME（composition）都這樣做。
- **上排拼音、下排候選字**：百度、QQ 的官方說明都明寫「上排是用户输入的拼音，下排是候选字词」。整句猜測只是第一個候選字，不是一串可以逐字點選的漢字。
- **整句漢字預覽**：小狼毫 `preedit_type: preview`、fcitx5 `CommitPreview`、鼠鬚管 `inline_candidate`、ibus-libpinyin（整句漢字＋底線）、舊版微軟拼音。
- **混合**：鼠鬚管程式碼註解示範了「已釘住的漢字＋[目前這段的候選]＋游標右側原始拼音」。

---

## 比較表

| 輸入法 | 編輯區顯示 | 候選字對應 | 改中段／尾端的方式 | 選字後游標 | 倒退鍵 | 送出後撤銷 |
|---|---|---|---|---|---|---|
| 搜狗 PC | 帶分隔符的拼音 | 游標前（1.0.1.6）；4.2 起游標在中間可直接編輯後段 | ← 回退、Shift+←/→ 選取、Ctrl+字母定位、以詞定字 `-` `=` | 【未驗證】 | 1.0.1.6 起「Undo 策略」；長按有緩衝不刪正文 | 【未找到】 |
| 搜狗手機 | 拼音串在候選字上方 | 【未驗證】 | 「拼音串可編輯」、滑動移游標、連續篩選 | 【未驗證】 | 【未驗證】 | 【未找到】 |
| 百度 PC | 上排拼音、下排候選字 | 游標前 | ← 以音節回退，先釘住前段再選後段；以詞定字 | 跳到剩下的拼音 | 長按只刪拼音 | 【未找到】 |
| 百度手機 | 輸入碼在候選字上方（可關） | 【未驗證】 | 「撤銷出錯漢字，直接編輯待上屏文字」、點擊游標區域、滑空白鍵 | 【未驗證】 | 【未驗證】 | 【未找到】 |
| QQ 拼音 PC | 拼音串＋候選字 | 游標前 | ← 定位、先釘住前段再選後段；以詞定字 | 跳到剩下的拼音 | 長按刪完拼音停 0.5 秒 | **Ctrl+Backspace 恢復上次輸入** |
| 手心 | 【未驗證】 | 【未驗證】 | Ctrl+字母、空白+字母、雙拼輔助碼定位修改；以詞定字 | 【未驗證】 | 【未驗證】 | 【未找到】 |
| 谷歌拼音（舊版桌面） | 【未驗證】（說明頁只有圖） | 【未驗證】 | 以詞定字 `[` `]`；「內嵌編輯模式」 | 【未驗證】 | 【未驗證】 | 【未找到】 |
| 微軟拼音（現行） | 【未驗證】 | 【未驗證】 | ←/→「在輸入區移動插入點」 | 【未驗證】 | 【未驗證】 | 【未找到】 |
| 微軟拼音 3.0／新體驗 | **整句漢字，加底線** | **游標所在的字** | ← 一次移一個漢字，候選窗打開，按數字換字 | 【未驗證】 | 【未驗證】 | 【未找到】 |
| macOS 簡體拼音 | 【未驗證】 | 【未驗證】 | 官方只記載 Shift-Delete「Undo the last substitution」 | 【未驗證】 | 【未驗證】 | 【未找到】 |
| RIME（rime-ice 設定） | 漢字＋拼音（小狼毫預設不內嵌、鼠鬚管預設內嵌） | 游標前 | ←（rime-ice 改成逐字）、Tab 跳音節、以詞定字（lua，**會先送出前段**） | 段延伸到游標就跳到尾端 | 先撤銷選字 | 無 |
| fcitx5 pinyin | 兩條（見前一份文件） | 游標前 | ←、點擊、以詞定字 `[` `]`（**留在編輯區**） | 剩下的拼音起點 | 先撤銷釘住 | 無 |
| ibus-libpinyin | 整句漢字＋底線；拼音在 aux | **游標在中間時對應游標後** | 把游標移到錯的音節前，直接選；前段不必先釘住 | 跳到剩下的拼音起點；到尾端直接送出 | 刪一個字元 | 無 |
| sunpinyin | 已釘住的漢字＋分音節拼音 | **永遠從第一個沒釘住的音節起**，和游標無關 | 只能由前往後一段一段選；Ctrl+數字刪候選 | 推到釘住範圍的後面；到尾端自動送出 | **先撤銷釘住**（`m_backspaceCancel` 預設開） | 無 |
| AOSP PinyinIME | 已釘住的漢字＋分音節拼音 | 釘住範圍之後 | 由前往後；按 ↑ 進 `EDIT_PINYIN` 改中間的拼音 | 【未查】 | 刪到釘住的字就整個音節解開 | 無 |
| fcitx5-android | 上行 aux＋編輯區（游標處畫 `\|`），下行 aux | 同 fcitx5 | 點 app 裡的編輯中文字 → fcitx5 點擊邏輯；滑空白鍵移游標 | 同 fcitx5 | 同 fcitx5 | 無 |
| Trime | 預設不內嵌；可設成編輯中文字或送出預覽 | 同 RIME | 點編輯區移動游標 | 同 RIME | 同 RIME | 無 |

---

## 游標不在最後時的 Enter／空白鍵

問題：游標停在句子中間（游標之後還有按鍵）時，按 Enter、空白鍵或選第一個候選字，會發生哪一種？

- **(a)** 送出全部：整句猜測，包含游標之後的部分。
- **(b)** 只送出游標之前的部分，其餘留在編輯區。
- **(c)** 只釘住游標之前的部分、不送出，接著游標跳到尾端（或候選字換成剩下的部分），繼續轉換。

另外也看：選了一個剛好吃掉游標前所有按鍵的候選字之後，游標會不會自動跳到尾端。

### 一覽

| 輸入法 | 空白鍵／選第一個候選字 | Enter | 選完游標前的部分後，游標 |
|---|---|---|---|
| RIME（`express_editor`，rime-ice 等大多數方案用這個） | **(c)**：釘住游標前那段；這段延伸到游標時，游標跳到尾端，剩下的接著轉換；釘住的段延伸到輸入尾端時直接送出 | 送出「已選定的漢字＋其餘的**原始拼音**」（`CommitRawInput`），不是整句猜測 | **跳到尾端** |
| RIME（`fluid_editor`） | **(c)**，同上；但延伸到輸入尾端時不送出，等 Enter | `CommitComposition`：先確認目前這段，確認後還有候選字就不送出（也就是 **(c)**），沒有才送出 | **跳到尾端** |
| fcitx5 pinyin | **(c)**：空白鍵選目前的候選字（游標前的 `candidatesToCursor`）；所有按鍵都被釘住時才送出 | 送出「已釘住的漢字＋其餘的原始拼音」（`CommitRawInput` 預設 Return 與各種修飾鍵組合） | **不移動**，停在釘住範圍的後面；但此時候選字換成「釘住範圍之後的全部」，效果接近 (c) |
| ibus-libpinyin | 空白鍵選目前高亮的候選字。游標在中間時候選字從**游標處**開始：選到的詞延伸到尾端就送出整句（前段用整句猜測）；沒到尾端就把游標移到剩下部分的起點 | 送出**全部原始拼音**（`m_text`） | 移到剩下部分的起點（不是尾端） |
| sunpinyin | **(a)**：空白鍵是 `makeSelection(0)`，第一個候選字是從釘住邊界到尾端的整句猜測（`BEST_TAIL`），選了直接送出整句；和游標位置無關 | 送出**全部原始拼音**（`_doCommit(false)`） | 選一般候選字時，游標被推到釘住範圍的後面；到尾端自動送出 |
| AOSP PinyinIME（`EDIT_PINYIN` 狀態，唯一有可見游標的狀態） | 空白鍵、Enter 都送出**編輯區顯示的字串**：已釘住的漢字＋其餘的拼音 | 同左；Enter 不是一般狀態時，送出目前候選字的整句並換行 | 不適用（這個狀態下不選字，↓ 回到選字） |
| 搜狗 PC 4.2 | 游標停在中間時「"搜狗"不变」，直接替游標**後面**選字，接近 (c) 但方向相反 | 【未驗證】 | 【未驗證】 |
| 百度 PC、QQ 拼音 PC | **(c)**：官方教學中，游標退回後選第一個候選字確定前段，候選字隨即換成剩下的拼音，接著選後段 | 【未驗證】 | 官方沒有寫游標位置，只寫候選字換成剩下的部分 |
| 微軟拼音（現行） | 【未驗證】 | 官方寫 Enter「without making a selection, but keeping the text you entered」，也就是不轉換、保留打的字；是送出拼音還是留在編輯區，原文不明確 | 【未驗證】 |

整體來看：
- **選字（空白鍵、數字鍵）在拼音輸入法裡大多是 (c)**：RIME、fcitx5、百度、QQ 都只釘住游標前的部分，不送出。RIME 明確把游標推到尾端，fcitx5 不推游標、但把候選字換成剩下的全部。
- **sunpinyin 是 (a)**，因為它的候選字根本不看游標。ibus-libpinyin 是另一個方向：候選字在游標後面。
- **Enter 在開源拼音輸入法裡幾乎都不是「送出整句猜測」**，而是送出原始拼音（ibus-libpinyin、sunpinyin），或「已選的漢字＋其餘原始拼音」（RIME `express_editor`、fcitx5、AOSP）。這和注音輸入法不同：注音輸入法的編輯區本來就顯示漢字，Enter 送出的就是畫面上的漢字。拼音輸入法裡想送出整句猜測，要按空白鍵或選第一個候選字。
- **本次沒有找到任何一家是 (b)**（只送出游標前、其餘留在編輯區）。最接近的是 rime-ice 的以詞定字 lua，它會送出前段、把後段放回輸入，但那是以詞定字的特例，不是 Enter 或空白鍵。

### 程式碼依據

**RIME。**
- 兩種 editor 的按鍵：空白鍵都是 `Confirm`；`fluid_editor` 的 Return 是 `CommitComposition`，`express_editor` 的 Return 是 `CommitRawInput`。`express_editor` 建構時把 `_auto_commit` 設成 true。
  來源：[editor.cc#L187-L215](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/editor.cc#L187-L215)、[#L41-L44](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/editor.cc#L41-L44)；rime-ice 用 `express_editor`：[rime_ice.schema.yaml#L52](https://github.com/iDvel/rime-ice/blob/927e1cf1d3eeb9bf4be85e3f684dba4dd485b6f6/rime_ice.schema.yaml#L52)
- `Confirm` 是 `ConfirmCurrentSelection() || Commit()`：有東西可以確認就只確認，不送出。`CommitComposition` 是「確認之後沒有候選字了才送出」。`CommitRawInput` 是先清掉沒確認的段，再送出。
  來源：[editor.cc#L87-L123](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/editor.cc#L87-L123)
- 確認之後的 `OnSelect`：
  - 這段延伸到輸入尾端：`_auto_commit` 時直接送出，否則開一個新的空段繼續。
  - 沒到尾端但已經到游標（`seg.end >= caret_pos`）：原始碼註解是「finished converting current segment, move caret to the end of input」，游標設到尾端。`set_caret_pos` 會觸發 `Compose`，剩下的接著轉換。
  - 還沒到游標：繼續轉換游標前剩下的部分。
  來源：[engine.cc#L340-L363](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/engine.cc#L340-L363)、[context.cc#L268-L274](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/context.cc#L268-L274)、[engine.cc#L197-L201](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/engine.cc#L197-L201)
- 送出的文字是「每段選定的候選字文字＋最後一段之後的原始輸入」，沒有選定的段送原始輸入。
  來源：[composition.cc#L102-L120](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/composition.cc#L102-L120)

**fcitx5 pinyin。**
- 空白鍵是 `CurrentCandidate`（選目前的候選字）；`CommitRawInput` 預設是 Return、KP_Enter 以及它們加 Ctrl、Shift 的組合，送出 `preeditCommitString`，也就是「`selectedSentence()`＋其餘的原始輸入」。
  來源：[pinyin.h#L269-L282](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.h#L269-L282)、[pinyin.cpp#L463-L474](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L463-L474)、[#L2320-L2325](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L2320-L2325)
- 釘住範圍延伸到輸入尾端（`selected()`）時，`updateUI` 直接送出整句。
  來源：[pinyin.cpp#L528-L544](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L528-L544)、[libime pinyincontext.cpp#L842-L854](https://github.com/fcitx/libime/blob/53df6a248793f9a00c1769e658630de313f3e419/src/libime/pinyin/pinyincontext.cpp#L842-L854)
- 選字之後，游標只有在落到釘住範圍之內時才被推到 `selectedLength()`，不會推到尾端。游標等於 `selectedLength()` 時，`needCandidatesToCursor()` 回傳 false，候選字改成「釘住範圍之後的全部」。所以選了剛好到游標的候選字後，游標停在原位，但候選字已經涵蓋游標之後的按鍵。
  來源：[libime pinyincontext.cpp#L170-L191](https://github.com/fcitx/libime/blob/53df6a248793f9a00c1769e658630de313f3e419/src/libime/pinyin/pinyincontext.cpp#L170-L191)、[#L837-L839](https://github.com/fcitx/libime/blob/53df6a248793f9a00c1769e658630de313f3e419/src/libime/pinyin/pinyincontext.cpp#L837-L839)

**ibus-libpinyin。**
- 空白鍵：有候選字就 `selectCandidate(cursorPos)`，否則送出。Enter：送出原始輸入 `m_text`。
  來源：[PYPPhoneticEditor.cc#L65-L105](https://github.com/libpinyin/ibus-libpinyin/blob/42788824ef59ffc03c6f345e20f08ee860dd5c15/src/PYPPhoneticEditor.cc#L65-L105)
- 選字後的游標見前面的 [libpinyin／ibus-libpinyin](#libpinyinibus-libpinyin候選字可以對應游標後面) 一節（`PYPLibPinyinCandidates.cc` L95-L177）。游標在中間時，第一個候選字是不是 NBEST 整句（會從位置 0 選起並直接送出）要看 `pinyin_guess_candidates` 在非 0 位置會不會產生 NBEST 候選，本次沒有查。**【未驗證】**

**sunpinyin。**
- 空白鍵 `makeSelection(0)`；Enter `_doCommit(false)`，送出 `getInputBuffer()` 的原始拼音。
  來源：[imi_view_classic.cpp#L255-L271](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp#L255-L271)、[#L539-L552](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp#L539-L552)
- `makeSelection`：`BEST_TAIL`（整句）直接 `selectSentence` 並送出；一般候選字把 `m_candiFrIdx` 推到詞尾、游標落後就跟著推，到尾端就送出。
  來源：[imi_view_classic.cpp#L659-L720](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp#L659-L720)
- 第一個候選字是 `BEST_TAIL`，依據是候選字清單的組法（L96-L134），沒有逐行追到排序。**【部分未驗證】**

**AOSP PinyinIME。**
- `processStateEditComposing`：空白鍵、DPAD_CENTER，以及一般狀態下的 Enter，都送出 `getComposingStr()`（`SHOW_STRING_LOWERCASE` 狀態則送出原始字串）；非一般狀態的 Enter 送出目前候選字的整句並換行。
  來源：[PinyinIME.java#617](https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679/src/com/android/inputmethod/pinyin/PinyinIME.java#617)（L617-L690）
- 一般輸入狀態（沒有可見游標）的 Enter：一般狀態送出原始拼音，否則送出整句並換行。
  來源：[PinyinIME.java#523](https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679/src/com/android/inputmethod/pinyin/PinyinIME.java#523)（L523-L535）

**閉源。**
- 百度：[百度輸入法幫助](http://www.baidu.com/search/shurufa_help.html) 第 6 節，← 回退後「选定"3"可确定"周一"部分」，再「选择"2.南"」。
- QQ：[pinyin05 #a22](https://skin.qq.pinyin.cn/help/pinyin05.html)，先「选择"1.桌球"」，再「选择"2.费"」。
- 搜狗：[升級日誌](https://pinyin.sogou.com/changelog.php) 4.2。
- 微軟拼音：[Microsoft Learn](https://learn.microsoft.com/globalization/input/simplified-chinese-ime) 的 Enter 說明。
- 這幾家都沒有寫 Enter 在游標停在中間時的行為。**【未驗證】**

### 對 mofoim 選 (c) 的意義

- mofoim 暫定的 (c)，和 RIME 的行為一致，而且 RIME 是唯一在原始碼裡明寫「這段到游標了，就把游標移到尾端」的。百度、QQ 的官方教學也是 (c) 的流程。可以放心說這是拼音輸入法的主流做法。
- 「游標要不要跳到尾端」有兩派：RIME 跳到尾端；fcitx5 不動游標，但把候選字範圍擴大到尾端。mofoim 的 ADR 0004 規定「游標之後的按鍵不參與猜測」，所以不能照抄 fcitx5（游標不動、候選字卻涵蓋游標之後），**要照 RIME 把游標推到尾端**，才不違反 ADR 0004。
- Enter 要特別想清楚。開源拼音輸入法的 Enter 大多是「送出原始拼音」，因為拼音字母本身就是有意義的英文輸出。mofoim 的按鍵序列是注音鍵位，原始按鍵沒有意義，ADR 0004 也已經寫明「送出一律送整個按鍵序列的結果：釘住的段，加上其餘所有按鍵（包含游標之後的）的整句猜測」，也就是 Enter 是 (a)。這和注音輸入法的習慣一致（編輯區看到的是什麼就送出什麼），但 mofoim 的編輯區顯示注音、不顯示整句猜測，使用者按 Enter 前看不到會送出什麼。這是 Enter 選 (a) 時要另外處理的顯示問題，例如加一條整句預覽（小狼毫 `preview`、fcitx5 `CommitPreview`、鼠鬚管 `inline_candidate`）。
- 如果空白鍵和數字鍵是 (c)、Enter 是 (a)，就和 RIME `express_editor`、fcitx5 一樣，「選字」和「送出」分給不同的鍵。差別只在 Enter 送出的是整句猜測，而不是原始按鍵。

---

## 各輸入法細節

### 搜狗拼音（Windows PC）

新版幫助中心是單頁應用，讀不到內文，所以主要的一手來源是官方升級日誌。
來源：[搜狗拼音升級日誌](https://pinyin.sogou.com/changelog.php)（GBK 編碼）

- **公測第二版 1.0.1.6（2006.6.21）**：「候选字计算只针对光标前面的拼音进行。」同一版還「优化了退格键Backspace的Undo功能（即后退时的删字符策略）」，並且「支持 Tab 键翻页」（要在選項中開啟）。
- **公測第四版 1.0.2.0（2006.8.15）**：
  - 「支持使用以词定字功能构造用户词」。
  - 同版修正了「立即转换模式下"-="以词定字键无效」的 bug，可見 `-` `=` 是以詞定字的按鍵之一。
  - 「显示拼音时添加了分隔符」：編輯區顯示帶分隔符的拼音。
- **4.2 正式版（2009.6.5）：游標回退後，前段不動，直接編輯後段。** 官方範例：
  - 輸入 `sogoushurufa`，第一個候選字是「搜狗输入法」。
  - 按 ← 把游標退到 `sogou` 和 `shurufa` 之間。
  - 原文：「"搜狗"不变，可以直接通过选词编辑"shurufa"匹配的选项」。
  - 這和 1.0.1.6 的「只針對游標前面」方向相反，也和百度、QQ 的教學方向相反，是本次找到的閉源輸入法裡唯一明寫「候選字對應游標後」的。前段是自動釘住、還是只是維持猜測，日誌沒有說。**【未驗證】**
- **5.0 正式版（2010.3.25）**：「按shift+左右方向键（←、→）即选中光标移过的拼音串」。選取之後能做什麼（刪除、重打），日誌沒有說。**【未驗證】**
- **5.2 正式版（2011.1.24）**：
  - 「支持Ctrl+字母进行首字母定位修改」。推測是跳到以該字母開頭的音節，官方沒有說明細節。**【未驗證】**
  - 「长按退格键有缓冲时间避免删除正文」。
- **學詞**：7.4（2014.11.24）「新增了学词时机，拼接修改后也学词」；7.4.1（2015.1.8）「退格修改后的词条也为您贴心保留」。也就是說，用分段或倒退鍵訂正出來的結果，會被學成新詞。
- **二手**：下載站 ZOL 說搜狗的以詞定字預設關閉（[來源](https://app.zol.com.cn/jiqiao/37474.html)）；少數派說新版搜狗、百度已拿掉以詞定字（[來源](https://sspai.com/post/84373)）。**【未驗證，二手文章】**
- 找不到：搜狗 macOS 版的任何一手說明（Mac 版升級日誌 404，主頁是單頁應用）；滑鼠點候選窗裡的拼音來定位；送出後撤銷。

### 搜狗手機輸入法

來源：[搜狗手機輸入法升級日誌](https://shouji.sogou.com/log.php)

- 2010.11.19：「将输入串显示在侯选词上方」；2011.03.04：「支持整句输入」。
- 2013.6.8：「手指在键盘上左右滑动，就可以移动光标及候选词条」。
- 2013.10.30：「新增连续筛选：音节连续筛选让整句输入信手拈来」。推測是九鍵左側的音節欄可以逐音節篩選。**【未驗證】**
- 2014.04.01：「新增纠错标记」，原文說要「告诉你哪里输错」。
- 2015.06.16、2015.07.07：九鍵增加分詞功能（設定中開啟），例如「西安」「迷你」可以一起打。
- **2016.03.25：「拼音串可编辑：拼音串哪里出错改哪里，无需清空再重来」。** 這是最接近 issue #2 的官方描述，但具體手勢（點拼音串插入游標？點已選的漢字退回？）沒有寫。**【未驗證】**
- 2021.08.13：「长句纠错优化」；同版「智能汪仔纠错」會找出句子中的錯別字，屬於送出後的校對。
- 2026.02：語音輸入的「的得地／他她它」一鍵糾錯。雖然是語音，正好是 issue #2「的／得」這類錯誤。
- iOS 版可以左右滑鍵盤移動游標，滑行輸入時無效。來源：[App Store 描述（廠商撰寫）](https://apps.apple.com/cn/app/id917670924)

### 百度輸入法（PC）

- **官方說明（©2010）**，來源：[百度輸入法幫助](http://www.baidu.com/search/shurufa_help.html)（GBK 編碼）
  - 介面：「上排是用户输入的拼音，下排是候选字词」，第一個詞預設紅色，按空白送出。
  - **第 6 節「如何手動構詞」，正好是「句尾猜錯」的教學。** 輸入 `zhouyinan`，最後一個字猜成「男」：
    1. 「按下"←"（向左键），光标可直接回退到"nan"音节前」。← 以音節為單位。
    2. 此時候選字是游標前 `zhouyi` 的候選，選 3「周一」把前段確定下來。
    3. 候選字換成剩下的 `nan`，選 2「南」。
    - 原文說明「手动构词功能在输入人名及较长整句时更常使用」。
  - 第 7 節：滑鼠停在候選字上，會出現「固定首位」「删除词条」。
- **百度用戶服務中心 FAQ**
  - [id=2524](https://help.baidu.com/question?prod_id=41&class=373&id=2524)：同樣寫「上排是用户输入的拼音，下排是候选字词」。
  - [id=2500](https://help.baidu.com/question?prod_id=41&class=373&id=2500)：以詞定字可選「逗号句号、减号等号或方括号」，前一個取首字，後一個取尾字；範例是「士兵」後按 `[` 得到「士」。
  - [id=2499](https://help.baidu.com/question?prod_id=41&class=373&id=2499)：「长按Backspace，输入法将只删除拼音，而不会误删除已上屏的内容」。
- **PC 更新日誌**，來源：[shurufa.baidu.com/update](https://shurufa.baidu.com/update/)
  - 2.2beta（2012.11.08）：「新增以词定字功能，先组词再用对应按键选择自己要的单字」。
  - 1.5.2beta（2011.09.28）：「优化智能编辑功能，修改更方便」，內容沒有說明。**【未驗證】**
  - 「新增动态纠错功能」，自動修正漏打、多打、錯打。
- 找不到：Ctrl+字母定位、Tab 分段、送出後撤銷。

### 百度手機輸入法

- **Android 版成長歷程**，來源：[srf.baidu.com/input/update.html](https://srf.baidu.com/input/update.html)
  - V3.2.5（2012-10-22）：「手势移动光标：左右点划空格，直接移动光标」。
  - **V3.3（2012-11-22）：「拼写修改：可以修改错误拼音，撤销出错汉字，直接编辑修改待上屏文字」。** 「撤銷出錯漢字」就是在送出前撤銷已經選錯的字。
  - V3.7（2013-06-08）：「拼写修改优化：优化光标点击区域」，表示是點擊編輯區來定位。
  - 「语音改错：点击错误区域，给出更多候选」。
  - 「左侧拼音连续选：支持九宫拼音码连续过滤」。
- **Android 說明（©2013）**，來源：[srf.baidu.com 說明](https://srf.baidu.com/?c=index&e=help)
  - 隔音符可以用「[分词]、[符]或者[符号]」，例如 `xian` 是「线」、`xi'an` 是「西安」。
  - 設定「整句输入」預設開；「显示输入字符」：輸入碼顯示在候選字上方。
  - 「智能纠错」。
- 具體手勢（點哪裡、點了之後候選字對應什麼）沒有找到一手說明。**【未驗證】**

### QQ 拼音輸入法（PC）

來源：[QQ 拼音幫助 pinyin05](https://skin.qq.pinyin.cn/help/pinyin05.html)、[pinyin03](https://skin.qq.pinyin.cn/help/pinyin03.html)、[pinyin04](https://skin.qq.pinyin.cn/help/pinyin04.html)

- **整句修改（pinyin05 #a22）**：前提是開啟「智能组词」。範例輸入 `zhuoqiufei`，發現「费」錯了：
  1. 「按"←"（向左键），光标定位到」錯的地方。
  2. 選 1「桌球」。
  3. 選 2「费」。
  - 原文說明「此功能在输入较长整句的时候更为适用」。模型和百度一樣：先釘住游標前段。
- **游標與高亮（pinyin03）**：「您可以通过方向键"←"和"→"来调整光标位置」；「通过方向键"↑"和"↓"来调整亮选的位置」；全拼下 `;` 也能調高亮。
- **以詞定字（#a34）**：在「按键设置」中設定，例如左括號取首字「瘫」、右括號取尾字「痪」。
- **恢復上次輸入（送出後撤銷）**：「按Ctrl+Backspace键恢复到选词上屏之前的场景，只要重新选词即可更正」。範例是誤送出「只是」，按 Ctrl+Backspace 後，原文說會「快速选中刚才输入的内容，并将相应的拼音串显示在输入框中」。
- **倒退鍵**：長按時，拼音串刪完會「停顿0.5秒」才繼續刪內文；「上屏后3秒内按退格键」，剛才誤造的詞不會進使用者詞庫。
- 允許用單引號或空白分隔音節；Tab 進入輔助碼（筆畫碼 h/s/p/z 或拆字首字母）。
- 介面（pinyin04）：「所输入的拼音串」和候選字都顯示在輸入窗口。
- 找不到：QQ 手機版的說明；PC 版更新日誌是動態載入，讀不到。

### 訊飛輸入法

- 官方說明（[srf.xunfei.cn/help](https://srf.xunfei.cn/help/)、[tips](https://srf.xunfei.cn/help/tips/?s=help)）只有模糊拼音、中英混打、刪除鍵左滑清空等，**沒有整句中段修正的說明**。
- 整句拼音漏打字母時，會在漏的地方用紅色倒 V 標出來，多打時有刪除標記。來源：[中國日報網評測](https://tech.chinadaily.com.cn/a/201911/22/WS5dd7753ba31099ab995ed822.html)。**【未驗證，新聞評測】**
- 「AI 校对」：先標出錯別字，再點「纠错」替換，屬於送出後。來源：[騰訊新聞](https://news.qq.com/rain/a/20211109A02OEA00)。**【未驗證，新聞報導】**

### 手心輸入法

- **Windows 更新日誌**，來源：[xinshuru.com/win_record.html](https://www.xinshuru.com/win_record.html)（資料來自 [pc.json](https://www.xinshuru.com/xinshuru/data/updatelogs/pc.json)）
  - 2014-05-23：「以词定字功能，先组词再用对应按键选择需要的字」。
  - 2014-07-11：「增加Tab键辅助码功能，支持笔画和部首拼音辅助过滤候选」。
  - 2014-11-03：「支持Ctrl+字母键定位修改功能」。
  - 2014-12-08：「双拼下支持辅助码定位修改功能」。
  - 2015-01-29：「增加"空格+字母"定位修改功能」。
- 「定位修改」的具體操作沒有說明。**【未驗證】**
- 手機說明（[xinshuru.com/help.html](http://www.xinshuru.com/help.html)）：九宮格左側拼音輔助區過濾候選、筆畫過濾。

### 谷歌拼音（舊版 Windows）

官方說明中心仍在：[support.google.com/pinyin](https://support.google.com/pinyin/?hl=zh-Hans)

- [組句](https://support.google.com/pinyin/answer/62645?hl=zh-Hans)：可以一次打一整句，「限制最长组句长度为 64 个拼音字符」。
- [以詞定字](https://support.google.com/pinyin/answer/69410?hl=zh-Hans)：在「属性设置」>「按键」>「选字框」>「以词定字」開啟，「使用左右方括号选择当前候选词的开头或结尾的字」；範例是「谷歌」按 `[` 得「谷」、按 `]` 得「歌」。
- [快捷鍵](https://support.google.com/pinyin/answer/79327?hl=zh-Hans)：上下方向鍵換候選字、Ctrl+Shift+數字刪除錯造的詞。**沒有列出 ←/→ 或分段編輯。**
- [翻頁](https://support.google.com/pinyin/answer/62659?hl=zh-Hans)：預設 `-` `=`。[二三候選](https://support.google.com/pinyin/answer/66269?hl=zh-Hans)：左右 Shift 或左右 Ctrl。
- [內嵌編輯模式](https://support.google.com/pinyin/answer/115052?hl=zh-Hans)（V2 測試版，另見 [115049](https://support.google.com/pinyin/answer/115049?hl=zh-Hans)）：原文是「所见即所得的编辑模式」，沒有寫怎麼操作。**【細節未驗證】**
- [智能糾錯](https://support.google.com/pinyin/answer/62661?hl=zh-Hans)：自動修正 mg→ng、iou→iu、uei→ui、uen→un、ve→ue。這是拼音拼法層級的容錯，和 mofoim 的誤觸容錯性質接近。

### 微軟拼音（現行 Windows 10/11）

- Microsoft Learn 頁面：Up/Down 換候選字、Space 選焦點候選字、數字直接選；Tab 展開視窗、顯示 Bing 建議；Enter 關閉候選窗、保留已打的拼音；Esc 刪掉已打的內容；打 `xiatian` 時輸入法會自動切成 `xia'tian`。設定裡有「super abbreviated Pinyin」「fuzzy Pinyin rules」「reading autocorrection」，按鍵可以自訂，但頁面沒有列出各個綁定。
  來源：[Microsoft Learn: Simplified Chinese IME](https://learn.microsoft.com/globalization/input/simplified-chinese-ime)
- 支援頁的快捷鍵表：拼音下 ←/→ 是「Move the insertion point to the left in the input area」；對照五筆是「only moves to the left within the unconverted input area」。另有「Use previous version of Microsoft Pinyin」相容選項。
  來源：[Microsoft Simplified Chinese IME（支援頁）](https://support.microsoft.com/en-us/windows/microsoft-simplified-chinese-ime-9b962a3b-2fa4-4f37-811c-b1886320dd72)
- 官方文件**沒有**描述：編輯區顯示拼音還是漢字、選了只涵蓋前段的候選字之後怎麼辦、倒退鍵、以詞定字、送出後撤銷。
- 依記憶：現行版的編輯區顯示以 `'` 分隔的拼音，漢字只在候選窗；選了前段的候選字後，前段變漢字、剩下的拼音繼續轉換。**【未驗證，依記憶】**

### 微軟拼音 3.0／新體驗風格（舊版）

- **官方逐步教學（Microsoft Pinyin IME 3.0，Windows XP）**，來源：[Microsoft Learn: Installing and Using Input Method Editors](https://learn.microsoft.com/en-us/windows/win32/dxtecharts/installing-and-using-input-method-editors)
  - 打完 `ni3` 後讀音窗消失，文字出現在 composition window，原文說「composition string is underlined」：**編輯區顯示的是轉好的漢字**。
  - 「Press the left arrow… The composition cursor moves one character to the left」：**游標以漢字為單位，移進已經轉好的字裡**，候選窗隨即為那個字打開。
  - 「Type '2' to select the second entry」，編輯區裡那個字就被換掉。
  - 按 Enter 送出。
  - 這是本次找到唯一有官方文字的「直接在漢字上改」。換字之後游標去哪、其他字會不會重算，教學沒有寫。**【未驗證】**
- Windows CE 文件說 MSPY 2.0「uses an intelligent, bigram-based language model. It is a sentence-based input method」。
  來源：[Microsoft Learn（Windows Embedded 舊文件）](https://learn.microsoft.com/en-us/previous-versions/windows/embedded/ms919343(v=msdn.10))
- 「新體驗」和「簡捷」兩種風格的差別，只找到二手來源，全部**【未驗證】**：
  - 2007 年語言列的「Natural」「Traditional」被「New Experience」「ABC」取代。來源：[Wake Forest 大學使用說明頁](https://yipcw.wfu.edu/atg/ime/2007/)（大學個人頁）。
  - 2010 版有更新的「New Experience」和新的「SimpleFast」風格，SimpleFast 以詞為單位、類似搜狗，是現行微軟拼音的基礎；New Experience 在 Windows 8 退役。來源：Pinyin Joe（愛好者網站，只讀到搜尋摘要）`https://pinyinjoe.com/faq/mspy2010-pinyin-ime-update.htm`。
  - 新體驗是內嵌的編輯區、自動轉換；簡捷是跟隨游標的小框、手動轉換。新體驗是「長句輸入＋二次確認」：第一次按空白後文字帶虛線底線，可以用 ←/→ 移動；第一個候選字錯了，按 → 跳回句首逐詞選。來源：jb51.net、ddooo.com 下載站文章（只讀到搜尋摘要）。
- 找不到：Office 2007/2010 或 MSPY 2010 的官方說明或 KB 文章（web.archive.org 抓不到）。

### Apple macOS 簡體拼音、iOS 拼音

- [候選字視窗](https://support.apple.com/guide/chinese-input-method/use-the-candidate-window-cim12992/mac)：數字鍵、雙擊或 Space 選；方向鍵換候選字；`=` `-` 或 `[` `]` 換列；拼音下 Option-Tab 依四個聲調篩選；Tab 選排序方式。**沒有提到怎麼改前面的字。**
- [拼音－簡體](https://support.apple.com/guide/chinese-input-method/cimpys11836/mac)：用 `'` 分隔有歧義的音節；可以全拼、簡拼混著打。
- [快捷鍵](https://support.apple.com/guide/chinese-input-method/cimkeys36647/mac)：Shift-Delete「Undo the last substitution」是唯一像撤銷的按鍵；沒有在段之間移動的按鍵。
- [設定](https://support.apple.com/guide/chinese-input-method/change-chinese-and-cantonese-settings-cim21aa5fa50/mac)：「Correct fuzzy pinyin」「Correct spelling」「Adjust word frequency」。
- [iOS 12 使用手冊](https://support.apple.com/guide/iphone/use-international-keyboards-iphadaaeb5f/12.0/ios)：點鍵盤上方的候選字；左滑看更多或點向上箭頭看完整列表。
- 依記憶：macOS 在 app 裡內嵌顯示拼音，選了只涵蓋前段的候選字後前段變漢字；iOS 拼音鍵盤左側有一欄音節／分詞的選擇。**【未驗證，依記憶】**

### Gboard 拼音

- 官方說明（[新增語言](https://support.google.com/gboard/answer/7068494)、[支援語言](https://support.google.com/gboard/answer/6380730)）只列出支援簡繁中文拼音，**沒有任何候選字、游標或訂正的說明**。
- 有部落格說長按候選字可以看拼音。**【未驗證，部落格】**

### RIME 周邊（不含 librime 核心）

**librime 預設按鍵補充。**
- navigator：`Left` 是 Rewind，`Ctrl+Left`/`Ctrl+Right` 以音節為單位，`KP_Left` 逐字元；可以綁的 action 有 `left_by_char`、`left_by_syllable`、`*_no_loop`、`home`、`end`。
  來源：[navigator.cc#L24-L64](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/navigator.cc#L24-L64)
- editor：`Ctrl+Delete` 是 `delete_candidate`；`Ctrl+Return` 送出原始輸入。
  來源：[editor.cc#L191-L215](https://github.com/rime/librime/blob/bb56876890ba60f446fa2640c244b1794b965eda/src/rime/gear/editor.cc#L191-L215)
- **以詞定字不在 librime 核心裡**（在 `src/` 裡找不到 `select_first_character`），是 schema 層的 lua 外掛。

**rime-ice／oh-my-rime 的以詞定字（`select_character.lua`）。**
- 按鍵：`key_binder/select_first_character: bracketleft`、`select_last_character: bracketright`。
  來源：[default.yaml#L208-L209](https://github.com/iDvel/rime-ice/blob/927e1cf1d3eeb9bf4be85e3f684dba4dd485b6f6/default.yaml#L208-L209)、掛載：[rime_ice.schema.yaml#L44](https://github.com/iDvel/rime-ice/blob/927e1cf1d3eeb9bf4be85e3f684dba4dd485b6f6/rime_ice.schema.yaml#L44)
- 做法：取目前候選字的首字或尾字，把 `get_commit_text()` 裡那個詞換成這個字，**直接 `engine:commit_text(part1)` 送出前半段**，然後 `context:clear()`，再把剩下的部分加上游標右側的原始輸入用 `push_input` 放回去。
  來源：[select_character.lua#L1-L60](https://github.com/iDvel/rime-ice/blob/927e1cf1d3eeb9bf4be85e3f684dba4dd485b6f6/lua/select_character.lua#L1-L60)
- 所以這是「送出前段、保留後段」，不是留在編輯區裡就地改。oh-my-rime 用同源腳本（上游 BlindingDark/rime-lua-select-character），預設也是 `[` `]`。
  來源：[oh-my-rime select_character.lua#L52-L60](https://github.com/Mintimate/oh-my-rime/blob/0cf3d63c61d07a84834c031bad4ef8ac39b322d0/lua/select_character.lua#L52-L60)

**rime-ice 的游標按鍵。**
- 把預設的 `Left: Rewind` 改成 `left_by_char_no_loop`，`Shift+Left/Right` 改成以音節移動。
  來源：[default.yaml#L90-L95](https://github.com/iDvel/rime-ice/blob/927e1cf1d3eeb9bf4be85e3f684dba4dd485b6f6/default.yaml#L90-L95)
- `Tab`/`Shift+Tab`（以及 `Alt+Left/Right`）被轉成 `Shift+Right/Left`，用來在音節之間跳。
  來源：[default.yaml#L212-L221](https://github.com/iDvel/rime-ice/blob/927e1cf1d3eeb9bf4be85e3f684dba4dd485b6f6/default.yaml#L212-L221)

**小狼毫（Weasel）。**
- 預設 `inline_preedit: false`、`preedit_type: composition`。
  來源：[weasel.yaml#L32-L33](https://github.com/rime/weasel/blob/f96ed24a8a535961b6ffc7160d98851a69cee4f8/output/data/weasel.yaml#L32-L33)
- `preedit_type` 有三種值：
  - `composition`：顯示 `composition.preedit`，帶 `sel_start`/`sel_end`/`cursor_pos`。
  - `preview`：改顯示 `commit_text_preview`（整句漢字），游標固定在尾端。
  - `preview_all`：在後面加上 `[ 候選1 候選2 … ]`，高亮的那個另外標記。
  來源：[RimeWithWeasel.cpp#L813-L880](https://github.com/rime/weasel/blob/f96ed24a8a535961b6ffc7160d98851a69cee4f8/RimeWithWeasel/RimeWithWeasel.cpp#L813-L880)、[#L1155-L1164](https://github.com/rime/weasel/blob/f96ed24a8a535961b6ffc7160d98851a69cee4f8/RimeWithWeasel/RimeWithWeasel.cpp#L1155-L1164)
- 滑鼠只能點候選字（`select_candidate_on_current_page`）、hover 高亮、翻頁。
  來源：[RimeWithWeasel.cpp#L314-L344](https://github.com/rime/weasel/blob/f96ed24a8a535961b6ffc7160d98851a69cee4f8/RimeWithWeasel/RimeWithWeasel.cpp#L314-L344)
- 沒找到「點編輯區移動游標」的程式碼。**【未驗證】**

**鼠鬚管（Squirrel）。**
- 預設 `inline_preedit: true`、`inline_candidate: false`；schema 選項 `no_inline`、`inline` 可以覆寫。
  來源：[squirrel.yaml#L36-L38](https://github.com/rime/squirrel/blob/b7970a93b05ae1ca1bcd97888fd6aa3cd0458d3e/data/squirrel.yaml#L36-L38)、[SquirrelInputController.swift#L461-L463](https://github.com/rime/squirrel/blob/b7970a93b05ae1ca1bcd97888fd6aa3cd0458d3e/sources/SquirrelInputController.swift#L461-L463)
- 程式碼註解完整示範了游標左移後的顯示：
  - 編輯區 `已選某些字[xiang zuo yi dong]|guangbiao`，`commit_text_preview` 是 `已選某些字向左移動`。
  - 開 `inline_candidate` 時，把游標後還沒轉換的拼音接在預覽後面，變成 `已選某些字[向左移動]|guangbiao`。
  - 也就是「已釘住的漢字＋[目前這段的候選]＋游標右側原始拼音」三段並排。
  來源：[SquirrelInputController.swift#L477-L525](https://github.com/rime/squirrel/blob/b7970a93b05ae1ca1bcd97888fd6aa3cd0458d3e/sources/SquirrelInputController.swift#L477-L525)

### fcitx5-chinese-addons pinyin（前一份文件沒寫的部分）

設定定義：[pinyin.h#L203-L320](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.h#L203-L320)。和訂正有關的選項：

| 選項 | 預設 | 意思 |
|---|---|---|
| `ChooseCharFromPhrase` | `[` | 以詞定字，取第 N 個字（註解說第 N 個鍵選第 N 個字） |
| `ChooseLastCharFromPhrase` | `]` | 以詞定字，取最後一個字 |
| `PrevCandidate`／`NextCandidate` | `Shift+Tab`／`Tab` | 換候選字 |
| `ForgetWord` | `Control+7` | 進入「遺忘詞」狀態 |
| `FilterByStroke` | `` ` `` | 筆畫篩選 |
| `CloudPinyinIndex` | 2 | 雲端候選字放在第幾個 |
| `PreeditCursorPositionAtBeginning` | 非 Android/Apple/Emscripten 時 true | 編輯區的游標畫在開頭 |

**以詞定字留在編輯區裡。** 取出候選字的第 idx 個字，算出這個候選字涵蓋的拼音長度，呼叫 `context_.selectCustom(segmentLength, chr)`：用一個字吃掉整段拼音，變成一個釘住的段，**不送出**。這和 rime-ice 的 lua 版不同。
來源：[pinyin.cpp#L2325-L2370](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L2325-L2370)、按鍵對應：[#L228-L237](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L228-L237)

**Left 和 Home。** 游標在釘住範圍的邊界（`cursor()==selectedLength()`）時，Left 或 Ctrl+Left 先 `cancel()` 最後一段，再移動；Home 把游標移到 `selectedLength()`，不是 0。
來源：[pinyin.cpp#L2291-L2318](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L2291-L2318)

**點擊編輯區（`invokeActionImpl`）。** 只處理 `LeftClick`，而且要 client 編輯區內容和引擎算的一致：
- 點的位置落在已釘住的漢字範圍內：反覆 `context.cancel()`，直到已釘住的文字長度不超過點擊位置，再把游標設到釘住範圍的後面。
- 點的位置落在拼音範圍內：從釘住範圍的後面開始，一格一格移動游標，直到對上點擊位置。
- 程式碼註解寫明這段邏輯要和 `preedit()` 一致。

來源：[pinyin.cpp#L2494-L2564](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L2494-L2564)

**ForgetWord。** 按 Ctrl+7 後，aux 顯示「[Select the word to remove from history]」，再用選字鍵選要遺忘的詞，從使用者詞庫和歷史中移除，然後重打原本的輸入。
來源：[pinyin.cpp#L1665-L1735](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L1665-L1735)、[#L1886-L1920](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L1886-L1920)

其他：筆畫篩選 [#L1788-L1830](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L1788-L1830)；雲端候選字插在 `cloudPinyinIndex-1` [#L618-L630](https://github.com/fcitx/fcitx5-chinese-addons/blob/e8d930f402e359cfd76a101ee3e46815d1331d83/im/pinyin/pinyin.cpp#L618-L630)。沒有找到送出後撤銷，只有標點的特例。

### libpinyin／ibus-libpinyin：候選字可以對應游標後面

**libpinyin 可以在任意位置加限制條件。** API 是 `pinyin_choose_candidate(instance, offset, candidate)`（回傳選完之後的位置）、`pinyin_clear_constraint(instance, offset)`、`pinyin_guess_candidates(instance, offset, sort)`。
來源：[pinyin.h#L544-L594](https://github.com/libpinyin/libpinyin/blob/85905e59cb19d0343cae84d831027aee113420b8/src/pinyin.h#L544-L594)、實作：[pinyin.cpp#L2501-L2650](https://github.com/libpinyin/libpinyin/blob/85905e59cb19d0343cae84d831027aee113420b8/src/pinyin.cpp#L2501-L2650)

- 一般候選字：`add_constraint(candidate->m_begin, candidate->m_end, token)`，回傳 `offset+len`。
- `add_constraint` 先清掉範圍內和它重疊的舊限制，在起點放 `CONSTRAINT_ONESTEP(token, end)`，其餘位置放 `CONSTRAINT_NOSEARCH`；之後 `pinyin_guess_sentence` 在限制周圍重新解碼。
  來源：[phonetic_lookup.cpp#L61-L118](https://github.com/libpinyin/libpinyin/blob/85905e59cb19d0343cae84d831027aee113420b8/src/lookup/phonetic_lookup.cpp#L61-L118)、[#L172-L211](https://github.com/libpinyin/libpinyin/blob/85905e59cb19d0343cae84d831027aee113420b8/src/lookup/phonetic_lookup.cpp#L172-L211)
- **NBEST 候選字**（整句的第 n 好結果）：用 `diff_result(best, other)` 把它和最佳整句不同的每一段都加成限制。等於「選一個整句，一次改掉所有不同的地方」。
- 換句話說，**libpinyin 的釘住可以在句子中間，前後兩邊照樣重算**。

**ibus-libpinyin 怎麼用。**
- 候選字的位置由 `getLookupCursor()` 決定：游標在尾端時用 `m_lookup_cursor`（隨著選字往前推）；**游標在中間時用游標所在的拼音位置**，也就是候選字從游標處開始列。
  來源：[PYPPhoneticEditor.cc#L345-L389](https://github.com/libpinyin/ibus-libpinyin/blob/42788824ef59ffc03c6f345e20f08ee860dd5c15/src/PYPPhoneticEditor.cc#L345-L389)
- 選字之後（[PYPLibPinyinCandidates.cc#L95-L177](https://github.com/libpinyin/ibus-libpinyin/blob/42788824ef59ffc03c6f345e20f08ee860dd5c15/src/PYPLibPinyinCandidates.cc#L95-L177)）：
  1. 在 lookup cursor 的位置 `pinyin_choose_candidate`，再 `pinyin_guess_sentence`。
  2. 選到的詞已經延伸到輸入的尾端，就學習並**直接送出整句**（前段用目前的整句猜測）。
  3. 還沒到尾端：游標原本在尾端時，`m_lookup_cursor` 推到剩下部分的起點；游標原本在中間時，**直接把游標移到剩下部分的起點**。
  - NBEST、LONGER 類候選字一律從位置 0 選起，並直接送出。
- 套到 issue #2：把游標移到 `de zi` 前面，候選字從 `de` 開始列，選「的字」就延伸到尾端，整句直接送出；前面的部分不必先選。
- 編輯區是「整句漢字＋還沒解析的原始輸入」，整段加底線，游標用 `pinyin_get_character_offset` 換算到漢字位置。
  來源：[PYPPinyinEditor.cc#L266-L294](https://github.com/libpinyin/ibus-libpinyin/blob/42788824ef59ffc03c6f345e20f08ee860dd5c15/src/PYPPinyinEditor.cc#L266-L294)
- ibus-libpinyin 沒有呼叫 `pinyin_clear_constraint`，所以 UI 上沒有「撤銷中間那個限制」的操作（在 `*.cc` 中搜尋為 0 筆）。

### sunpinyin（`CIMIClassicView`）

檔案：[imi_view_classic.cpp](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp)

- **兩個位置**：`m_cursorFrIdx` 是游標，`m_candiFrIdx` 是釘住和沒釘住的分界。**候選字永遠從 `m_candiFrIdx` 開始**（`getCandidates(m_candiFrIdx, …)`，[L519-L523](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp#L519-L523)），和游標在哪裡無關。游標只用來編輯拼音。
- 每次更新都算出整句猜測（`BEST_TAIL`）和其他的整句結果（`OTHER_BEST_TAIL`），放進候選字。[L96-L134](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp#L96-L134)
- 按鍵（[L180-L296](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp#L180-L296)）：Left/Right 以音節移動，Ctrl+Left/Right 逐字元；Enter 送出拼音原文；**Ctrl+數字刪除候選字**（[imi_view.h#L128-L132](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view.h#L128-L132)、預設修飾鍵 [imi_view.cpp#L54](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view.cpp#L54)）。
- 游標在分界上按 Left，會 `cancelSelection`；Home 撤銷全部釘住。[L554-L656](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp#L554-L656)
- 選字後 `m_candiFrIdx = candi.m_end`，游標落後就推過去；到尾端自動送出；選整句猜測直接送出。[L659-L720](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp#L659-L720)
- 倒退鍵：`m_backspaceCancel` 預設 true（[imi_view.cpp#L65](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view.cpp#L65)），只要有釘住的段，就先撤銷，不刪拼音。[L484-L517](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp#L484-L517)
- 編輯區：釘住的部分顯示漢字，其餘是以空白分音節的拼音。[L367-L420](https://github.com/sunpinyin/sunpinyin/blob/e4be7e64e4a168a684b26679b7e076118a2c97f7/src/ime-core/imi_view_classic.cpp#L367-L420)
- 結論：要改句尾，只能先把前面一段一段選完，或者在 `OTHER_BEST_TAIL` 裡剛好有對的整句。

### Google 拼音 Android 開源版（AOSP PinyinIME）

連結格式：`https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679/<路徑>#<行號>`

- **`ComposingView` 三種狀態**（[ComposingView.java#38](https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679/src/com/android/inputmethod/pinyin/ComposingView.java#38)，L38-L75、L143-L187）：
  - `SHOW_PINYIN`：一般打字。
  - `SHOW_STRING_LOWERCASE`：沒有釘住的字時按 ↑，Enter 送出英文小寫。
  - `EDIT_PINYIN`：已經有釘住的字時按 ↑，出現可見的游標，可以改中間的拼音；←/→ 呼叫 `mDecInfo.moveCursor(±1)`。↓ 回到 `SHOW_PINYIN`。
- **顯示字串**是「整句的前 `mFixedLen` 個漢字（釘住的部分）＋剩下的拼音（音節以空白分隔）＋還沒解析的尾巴」。`mFixedLen` 來自引擎的 `imGetFixedLen()`。
  來源：[PinyinIME.java#1812](https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679/src/com/android/inputmethod/pinyin/PinyinIME.java#1812)（L1812-L1878）
- **選字**：`chooseDecodingCandidate` 呼叫 `imChoose(candId)`，引擎從釘住位置之後重新搜尋。[PinyinIME.java#1773](https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679/src/com/android/inputmethod/pinyin/PinyinIME.java#1773)（L1773-L1810）
- **游標**在釘住的漢字範圍內一次跳一整個音節。[PinyinIME.java#2087](https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679/src/com/android/inputmethod/pinyin/PinyinIME.java#2087)（L2087-L2127）
- **刪除**：游標落在釘住的漢字範圍時，刪掉一整個音節；否則刪一個拼音字母。[PinyinIME.java#1668](https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679/src/com/android/inputmethod/pinyin/PinyinIME.java#1668)（L1668-L1690）。引擎的 `clear_fixed_this_step` 參數決定是否解開最後一個釘住的詞。[matrixsearch.h#405](https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679/jni/include/matrixsearch.h#405)
- 引擎有 `cancel_last_choice()`（[matrixsearch.h#444](https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679/jni/include/matrixsearch.h#444)），也包成 `imCancelLastChoice`，但 Java 介面沒有呼叫它。
- 選字狀態下 ←/→ 移動候選字高亮，↑/↓ 翻頁，刪除鍵直接回到 idle。（PinyinIME.java L573-L612）
- 結論：同樣只能由前往後釘住；但 `EDIT_PINYIN` 讓使用者在釘住之後還能回頭改中間的拼音。

### Trime（同文，Android 上的 RIME）

- `InlinePreeditMode`：DISABLE（預設）、COMPOSING_TEXT、COMMIT_TEXT_PREVIEW。
  來源：[AppPrefs.kt#L102](https://github.com/osfans/trime/blob/2e2b4d5394297745bf42ff6260bb547a271f378a/app/src/main/java/com/osfans/trime/data/prefs/AppPrefs.kt#L102)、[Rime.kt#L337-L355](https://github.com/osfans/trime/blob/2e2b4d5394297745bf42ff6260bb547a271f378a/app/src/main/java/com/osfans/trime/core/Rime.kt#L337-L355)
- **點編輯區移動游標**：`PreeditTextView.onTouchEvent` 用 `getOffsetForPosition` 算出點擊位置，ACTION_UP 時經 `rime.moveCursorPos` 呼叫 `set_caret_pos`。
  來源：[PreeditTextView.kt#L22-L57](https://github.com/osfans/trime/blob/2e2b4d5394297745bf42ff6260bb547a271f378a/app/src/main/java/com/osfans/trime/ime/composition/PreeditTextView.kt#L22-L57)、[Rime.kt#L221-L224](https://github.com/osfans/trime/blob/2e2b4d5394297745bf42ff6260bb547a271f378a/app/src/main/java/com/osfans/trime/core/Rime.kt#L221-L224)
- 傳進去的是編輯區字串的位置，`set_caret_pos` 吃的是原始輸入的位置，沒看到兩者之間的換算；編輯區裡有已轉成漢字的段時，位置可能會偏。**【未驗證，閱讀程式碼的推論】**

### fcitx5-android

- 編輯區上下兩行：上行是 auxUp＋編輯區，游標處插入 `|`；下行是 auxDown。
  來源：[PreeditUi.kt#L70-L104](https://github.com/fcitx5-android/fcitx5-android/blob/eb5b50216e70484a009933d9519db76005f1faf3/app/src/main/java/org/fcitx/fcitx5/android/input/preedit/PreeditUi.kt#L70-L104)
- 空白鍵左右滑動送出 Left/Right（`space_swipe_move_cursor` 預設開）。
  來源：[BaseKeyboard.kt#L154-L176](https://github.com/fcitx5-android/fcitx5-android/blob/eb5b50216e70484a009933d9519db76005f1faf3/app/src/main/java/org/fcitx/fcitx5/android/input/keyboard/BaseKeyboard.kt#L154-L176)
- **點 app 輸入框裡的編輯中文字**：系統游標落在編輯中範圍內時，換算成字元位置，送出 `InvokeActionEvent(LeftClick, position)`，最後走到上面 fcitx5 pinyin 的 `invokeActionImpl`，撤銷到點擊處或把游標移過去。這是本次找到最完整的「點中間的字 → 回到那裡重選」路徑。Android 上 `PreeditMode` 預設是 No，要改成有 client 編輯區才會生效。
  來源：[FcitxInputMethodService.kt#L889-L903](https://github.com/fcitx5-android/fcitx5-android/blob/eb5b50216e70484a009933d9519db76005f1faf3/app/src/main/java/org/fcitx/fcitx5/android/input/FcitxInputMethodService.kt#L889-L903)、[androidfrontend.cpp#L436-L440](https://github.com/fcitx5-android/fcitx5-android/blob/eb5b50216e70484a009933d9519db76005f1faf3/app/src/cpp/androidfrontend/androidfrontend.cpp#L436-L440)

### 其他（沒有查）

倉輸入法（Hamster，iOS 上的 RIME）、ibus-rime、Yong、fcitx4 的 sunpinyin 外掛都沒有查原始碼。推測前兩者只是轉發 librime 的編輯區和游標，fcitx4-sunpinyin 沿用 `CIMIClassicView`。**【未驗證，推測】**

---

## 對 mofoim 的啟示

### issue #2 的情境在 mofoim 現在要怎麼改

ADR 0004 的模型，和百度、QQ 官方教學、RIME、fcitx5、sunpinyin 是同一類（模式 A）：
- 候選字只對應游標前、還沒釘住的按鍵；
- 只能從還沒釘住的按鍵最前面開始釘住；
- 游標之後的按鍵不參與猜測。

所以改「的字」的步驟是：把游標往前移兩個音節，叫出候選字選第一個（前段的整句猜測）把前段釘住，再把游標移回尾端，重新叫出候選字選「的字」。百度、QQ 的教學也是這幾步，差別是它們把「游標回到尾端」這一步省掉了：前段選完後候選字自動換成剩下的拼音。

mofoim 比其他拼音輸入法多一個麻煩：容錯可能讓「de zi」本身的讀音被解讀錯（例如其中一個鍵被當成相鄰鍵）。其他輸入法只要在同音字裡挑，mofoim 的候選字還要能提供「另一種讀音解讀」（前一份文件的 E 點）。

### 不必改 ADR 0004 就能用的做法

1. **選了只涵蓋前段的候選字之後，游標自動跳到尾端。** 百度、QQ、sunpinyin、AOSP、ibus-libpinyin 選完前段後，候選字都直接換成剩下的部分，RIME 則是「這段延伸到游標就跳到尾端」（`engine.cc` L340-L363，見前一份文件）。mofoim 如果在「游標之後還有按鍵」時，選完把游標推到尾端（或推到下一個釘住的邊界），issue #2 就少掉「再把游標移回去」這一步。這只改選字後游標的位置，不改釘住規則。
2. **一鍵釘住游標前的整句猜測。** 搜狗 4.2 的「前段不變、直接編輯後段」，在 mofoim 可以拆成：使用者把游標停在「de」前面，按一個鍵，就把游標前的整句猜測釘住、游標跳到尾端，接著候選字就是「de zi」的。釘住仍然是從前面開始，只是一次做完兩步。
3. **以詞定字（`[` `]`）。** 候選字對應的範圍不變，只是讓使用者從較長的候選字裡取首字或尾字。fcitx5 的做法（`selectCustom`，留在編輯區當成一個釘住的段）比 rime-ice 的做法（直接送出前段）更符合 mofoim，因為 mofoim 的「送出」一律送整個按鍵序列。注意：在大千注音上，`[` `]` 本身不是注音符號，可以拿來用；但要確認鍵盤配置裡沒有把它們當成相鄰鍵。
4. **點擊編輯區定位。** fcitx5 的 `invokeActionImpl` 規則完全符合 ADR 0004：點到釘住的段就撤銷釘住，直到涵蓋點擊位置；點到注音就把游標移過去。mofoim 的 web 前端自己畫編輯區，做起來最直接，手機上也是主要的訂正方式（百度手機 V3.7「优化光标点击区域」、搜狗手機「拼音串可编辑」）。
5. **快速定位。** 搜狗 5.2、手心的「Ctrl+字母定位修改」：跳到某個音節。mofoim 可以用「Ctrl+注音符號」跳到以那個符號開頭的音節。這是純游標操作。
6. **候選字裡放整句的其他猜法**（libpinyin NBEST、sunpinyin `OTHER_BEST_TAIL`、新酷音 Tab）。第二好的整句可能剛好只差在「的字／得自」。這類候選字涵蓋游標前所有沒釘住的按鍵，和 ADR 0004「第一個候選字是整句猜測」相容，只是多放幾個整句。
7. **送出後撤銷**（QQ 拼音 Ctrl+Backspace，原文「恢复到选词上屏之前的场景」）。mofoim 本來就保留整個按鍵序列，送出後留著上一次的按鍵序列和釘住的段，撤銷時還原到編輯區即可。web 前端掌握文字區，可以把剛送出的文字刪掉。這也和「送出後引擎不再更動」不衝突，因為是重新開一個編輯區。
8. **行動手勢**：空白鍵左右滑動移游標（搜狗手機、百度手機、fcitx5-android）。
9. **倒退鍵先撤銷釘住**：ADR 0004 已經這樣做（在釘住的段邊界按倒退鍵撤銷），sunpinyin 的 `m_backspaceCancel` 更激進：只要有釘住的段就先撤銷。mofoim 目前的做法比較接近 fcitx5，可以維持。

### 需要「在句子中間釘住」才能做的做法

1. **候選字對應游標之後**（ibus-libpinyin、搜狗 4.2）。游標停在「de」前面時，候選字從「de」開始列，選了「的字」就在句子中間加一個限制，前段維持整句猜測、不必釘住。libpinyin 證明了引擎層做得到：限制條件可以放在任意位置，加新限制時清掉重疊的舊限制，前後重新解碼。對 mofoim 來說這要改掉 ADR 0004 的三件事：
   - 「游標之後的按鍵不參與猜測」：至少要對游標之後的按鍵提出候選字。
   - 「釘住只能從前面開始」：釘住的段可以在中間，前後都還是沒釘住的按鍵。
   - 「釘住的段只提供最後一個詞當作前文」：中間的釘住段，前後兩邊的猜測都要以它為條件，也就是回到 ADR 0003 那種「釘住是圖上的限制條件」。
   - 另外要定義「改了釘住段裡的按鍵」或「游標移進中間的釘住段」時怎麼辦。新酷音的做法是清掉跨過插入點的釘住（見前一份文件）。
2. **直接在漢字上改**（舊版微軟拼音 3.0、新體驗）。編輯區顯示整句漢字，游標以字為單位，候選窗對應游標所在的字。這除了要在中間釘住，還要在漢字和按鍵之間對位置，正是 ADR 0004 刻意避開的事；而且 mofoim 的編輯區照實際按的鍵顯示注音（`GLOSSARY.md` 的定義），要做就得改這個定義。微軟自己後來也放棄了新體驗風格（**【未驗證】**，二手來源說 Windows 8 退役）。
3. **整句其他猜法的「差異釘住」**（libpinyin NBEST 的 `diff_result`）。選一個整句，只把和最佳整句不同的幾段釘住。如果只當成「把整句全部釘住」，在 ADR 0004 下可以用（上面第 6 點）；要只釘住差異的部分，就需要中間釘住。

### 建議

- issue #2 最小的改善，可以先做「不必改 ADR 0004」那一組裡的 1、2、4：選了前段之後游標自動跳到尾端、一鍵釘住游標前的整句猜測、點擊編輯區定位。三者合起來，改句尾錯字的流程就和百度、QQ 的官方教學一樣短。
- 「候選字對應游標之後」是唯一能做到「完全不碰前段」的做法，ibus-libpinyin 證明它在引擎層可行，但要推翻 ADR 0004 的核心取捨。如果要做，應該另寫 ADR，並參考 libpinyin 的限制陣列（`CONSTRAINT_ONESTEP`／`CONSTRAINT_NOSEARCH`）。
- 本次沒有找到任何拼音輸入法在編輯區標出「這裡可能打錯」的一手說明，只有搜狗手機的「纠错标记」（日誌只有一句話）和訊飛的紅色倒 V（新聞評測）。這仍然是 mofoim 可以差異化的地方。

---

## 未驗證的主張

- **搜狗 PC**：4.2 版游標停在中間時，前段是被釘住還是只是維持猜測；5.0 Shift+←/→ 選取拼音串之後能做什麼；5.2「Ctrl+字母定位」的具體語意；以詞定字預設關閉、新版已移除（ZOL、少數派二手文章）。
- **搜狗 Mac**：完全找不到一手資料。
- **搜狗手機**：「拼音串可编辑」「连续筛选」的具體手勢；「纠错标记」怎麼顯示。
- **百度手機**：「撤销出错汉字」「光标点击区域」的具體手勢與候選字對應。
- **百度 PC**：「智能编辑」的內容。
- **手心**：「定位修改」的具體語意。
- **訊飛**：紅色倒 V 漏字標記、AI 校對（新聞評測與報導）。
- **谷歌拼音**：「內嵌編輯模式」怎麼操作；編輯區顯示什麼（說明頁只有圖）。
- **微軟拼音（現行）**：編輯區顯示拼音、選了前段後的流程、倒退鍵、以詞定字、Ctrl+Shift+F 繁簡切換（官方文件都沒有寫，依記憶）。
- **微軟拼音 3.0／新體驗**：換字之後游標的位置與其他字會不會重算；新體驗與簡捷的差別、新體驗在 Windows 8 退役（Pinyin Joe、jb51.net、ddooo.com、大學個人頁等二手來源，且部分只讀到搜尋摘要）。Office 2007/2010 的官方說明因 web.archive.org 無法存取而沒有查到。
- **macOS 簡體拼音、iOS 拼音**：編輯區顯示方式、選了前段之後的流程、iOS 左側音節欄（依記憶）。
- **Gboard**：長按候選字看拼音（部落格）；其他行為完全沒有官方說明。
- **小狼毫**：點編輯區不能移動游標（沒找到相關程式碼，不代表確定不存在）。
- **Trime**：點擊位置與 `set_caret_pos` 之間沒有換算、可能會偏（閱讀程式碼的推論）。
- **倉輸入法、ibus-rime、Yong、fcitx4-sunpinyin**：沒有查原始碼。
- **游標不在最後時的 Enter／空白鍵**：百度、QQ、搜狗、微軟拼音的 Enter 行為；ibus-libpinyin 游標在中間時第一個候選字是不是 NBEST 整句；sunpinyin 第一個候選字一定是 `BEST_TAIL`。
- **送出後撤銷**：除了 QQ 拼音，本次調查的開源拼音輸入法都沒有找到，閉源的也沒有找到官方說明。

## 來源列表

**原始碼**
- librime：https://github.com/rime/librime/tree/bb56876890ba60f446fa2640c244b1794b965eda
- weasel：https://github.com/rime/weasel/tree/f96ed24a8a535961b6ffc7160d98851a69cee4f8
- squirrel：https://github.com/rime/squirrel/tree/b7970a93b05ae1ca1bcd97888fd6aa3cd0458d3e
- rime-ice：https://github.com/iDvel/rime-ice/tree/927e1cf1d3eeb9bf4be85e3f684dba4dd485b6f6
- oh-my-rime：https://github.com/Mintimate/oh-my-rime/tree/0cf3d63c61d07a84834c031bad4ef8ac39b322d0
- fcitx5-chinese-addons：https://github.com/fcitx/fcitx5-chinese-addons/tree/e8d930f402e359cfd76a101ee3e46815d1331d83
- libime：https://github.com/fcitx/libime/tree/53df6a248793f9a00c1769e658630de313f3e419
- libpinyin：https://github.com/libpinyin/libpinyin/tree/85905e59cb19d0343cae84d831027aee113420b8
- ibus-libpinyin：https://github.com/libpinyin/ibus-libpinyin/tree/42788824ef59ffc03c6f345e20f08ee860dd5c15
- sunpinyin：https://github.com/sunpinyin/sunpinyin/tree/e4be7e64e4a168a684b26679b7e076118a2c97f7
- AOSP PinyinIME：https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/+/49aebad1c1cfbbcaa9288ffed5161e79e57c3679
- trime：https://github.com/osfans/trime/tree/2e2b4d5394297745bf42ff6260bb547a271f378a
- fcitx5-android：https://github.com/fcitx5-android/fcitx5-android/tree/eb5b50216e70484a009933d9519db76005f1faf3

**官方文件、說明頁、升級日誌**
- 搜狗拼音升級日誌：https://pinyin.sogou.com/changelog.php
- 搜狗手機輸入法升級日誌：https://shouji.sogou.com/log.php
- 搜狗輸入法 iOS App Store 描述：https://apps.apple.com/cn/app/id917670924
- 百度輸入法幫助（PC）：http://www.baidu.com/search/shurufa_help.html
- 百度用戶服務中心 FAQ：https://help.baidu.com/question?prod_id=41&class=373&id=2524 、https://help.baidu.com/question?prod_id=41&class=373&id=2500 、https://help.baidu.com/question?prod_id=41&class=373&id=2499
- 百度輸入法 PC 更新日誌：https://shurufa.baidu.com/update/
- 百度手機輸入法成長歷程：https://srf.baidu.com/input/update.html
- 百度手機輸入法說明：https://srf.baidu.com/?c=index&e=help
- QQ 拼音幫助：https://skin.qq.pinyin.cn/help/pinyin03.html 、https://skin.qq.pinyin.cn/help/pinyin04.html 、https://skin.qq.pinyin.cn/help/pinyin05.html
- 訊飛輸入法幫助：https://srf.xunfei.cn/help/ 、https://srf.xunfei.cn/help/tips/?s=help
- 手心輸入法更新日誌：https://www.xinshuru.com/win_record.html （資料 https://www.xinshuru.com/xinshuru/data/updatelogs/pc.json ）
- 手心輸入法手機說明：http://www.xinshuru.com/help.html
- 谷歌拼音說明中心：https://support.google.com/pinyin/?hl=zh-Hans （answer 62645、69410、79327、62659、66269、115052、115049、62630、62661）
- Microsoft Learn: Simplified Chinese IME：https://learn.microsoft.com/globalization/input/simplified-chinese-ime
- Microsoft Simplified Chinese IME（支援頁）：https://support.microsoft.com/en-us/windows/microsoft-simplified-chinese-ime-9b962a3b-2fa4-4f37-811c-b1886320dd72
- Microsoft Learn: Installing and Using Input Method Editors（MSPY 3.0 教學）：https://learn.microsoft.com/en-us/windows/win32/dxtecharts/installing-and-using-input-method-editors
- Microsoft Learn（Windows Embedded，MSPY 2.0）：https://learn.microsoft.com/en-us/previous-versions/windows/embedded/ms919343(v=msdn.10)
- Apple 中文輸入法使用手冊：https://support.apple.com/guide/chinese-input-method/use-the-candidate-window-cim12992/mac 、https://support.apple.com/guide/chinese-input-method/cimpys11836/mac 、https://support.apple.com/guide/chinese-input-method/cimkeys36647/mac 、https://support.apple.com/guide/chinese-input-method/change-chinese-and-cantonese-settings-cim21aa5fa50/mac
- Apple iPhone 使用手冊（iOS 12）：https://support.apple.com/guide/iphone/use-international-keyboards-iphadaaeb5f/12.0/ios
- Gboard 說明：https://support.google.com/gboard/answer/7068494 、https://support.google.com/gboard/answer/6380730

**二手來源（只用於【未驗證】的主張）**
- ZOL：https://app.zol.com.cn/jiqiao/37474.html
- 少數派：https://sspai.com/post/84373
- 中國日報網：https://tech.chinadaily.com.cn/a/201911/22/WS5dd7753ba31099ab995ed822.html
- 騰訊新聞：https://news.qq.com/rain/a/20211109A02OEA00
- Wake Forest 大學頁面：https://yipcw.wfu.edu/atg/ime/2007/
- Pinyin Joe：https://pinyinjoe.com/faq/mspy2010-pinyin-ime-update.htm
- jb51.net：https://www.jb51.net/os/Windows8/311852.html
