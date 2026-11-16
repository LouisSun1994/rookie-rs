# Experience 03 — 遊戲引擎、Bevy 與 ECS：想獨立做一款遊戲，引擎到底是什麼、該挑哪個

> 經驗不是筆記：起點是一個實際撞到的問題，往下挖到能解釋它為止。這一篇撞到的不是錯誤訊息，是一個卡住的決定：想獨立做一款遊戲，卻說不清「引擎」到底是什麼、該挑哪個。從引擎一覽與費用起步，經過「code agent 時代引擎還有沒有差」，落到 Bevy（Rust 寫的 ECS 引擎）的現況、ECS 的模型、版本怎麼鎖，最後定下第一款做貪吃蛇，並把 macOS 上從零到第一個視窗的建置步驟排出來。
> 核心收穫是四句話：**引擎 = 執行期的一包函式庫（runtime）+ 開發期的工具鏈**，兩半各有各的平台；**引擎很少是成敗的瓶頸，但目標平台、遊戲類型、誰是寫程式的主力這三個變數會改變答案**；**ECS 是一個每幀被查詢和更新的記憶體內資料庫**，system 只是函式，跑它的是 executor；**鎖住版本的是 `Cargo.lock`，不是 `Cargo.toml`**。
> 照步驟在 Mac 上實跑之後，又順著 `cargo run` 把建置那一半往下挖：cargo、rustc、linker 三個 process 怎麼接力、rustc 的多層 IR 與 LLVM（§11），以及 crate、feature、ABI、靜態與動態連結這幾個名詞各指什麼（§12）。
> 當時定位：🥚 小白，第一次準備動手寫程式。
> 起因：想獨立做一款遊戲，第一步選引擎就卡住：有哪些、要不要錢、「Godot 是 PC 端的嗎」、code agent 都能寫程式了引擎還有沒有差。
> 接在哪裡：[note 03](../notes/03-gpu-stack-driver-kernel.md)（「引擎」這個詞、GPU 那條鏈的下半段）、[note 04 §4](../notes/04-borrow-exclusivity-lifetime-elf.md)（多讀 XOR 單寫）、[經驗 01 §3–§5、§7](01-libc-abi-alpine.md)（API 與 ABI、ISA、target triple、動態載入器）、[note 05 §13](../notes/05-macos-sandbox-seatbelt-proc-cred.md)（Mach）。OS 藍圖：子系統 ①（一個 process 裡的 main thread、worker、Render thread）、②（dyld 載入 dylib 與 framework）。
> 時效：Bevy 0.20 於 2026-10-08 釋出，`cargo add bevy@0.20` 解析到 0.20.1；Godot 穩定版 4.7（2026-06）；各引擎的授權費用。皆為 2026-11 時的狀態，費用以官網為準。
> 來源與驗證狀態：§3–§9 來自這條線的對話。§2 分層與 §13 思維鏈是整理時由導師補上的骨架，對話裡沒有逐條走過；其中執行緒與 schedule 的順序對照過 Bevy 0.20.1 原始碼。§10 的兩段程式碼與 `Cargo.toml` 在 Linux 上對 bevy 0.20.1 跑過 `cargo check`（rustc 1.97.0 加 `--ignore-rust-version`）；§10 的步驟 5–10、13 在 2026-11-13 到 11-16 於 Mac 上做過（專案名 `bevy-snake`，rustc 1.100.0，`src/main.rs` 用的是觀察用範例）：建置成功、產出執行檔；視窗與 `AdapterInfo` 那一行沒有記錄下來，其餘步驟與標「推論」的仍是依機制推出來、沒有重現過的。§1 事件與 §14 解法對照是整理進 repo 時依本文其他章節重排的，沒有新增內容。§10 步驟 1 的「為什麼要裝它」、§1 的 t7 與陷阱 21–24 是入庫當天的追問，在整理 repo 的 session 裡問答，不是原本那條線的導師，也沒有在 Mac 上逐項驗證。§11 是 2026-11-16 入庫的另一份學習草稿（筆記形式，講 toolchain 與編譯流程），內文照原樣，只把章節降一層、重新編號，它的陷阱併入 §15 的 25–33；其中兩則 `> **補強**` 是入庫時加的。§12、§1 的 t9、陷阱 34–43 與 §10 排錯表後的補強是同一天的追問，同樣在整理 repo 的 session 裡問答，不是原本那條線的導師；標「實測」的是在這台 Mac 上量到的，其餘是依機制的說明，待 /校正。沒有腦筋急轉彎留在本文；待實跑確認的推論與追問留下的題目記在 [`blueprints/os-checklist.md`](../blueprints/os-checklist.md)。

---

## 0. 一句話版本

遊戲引擎是兩樣東西：連結進你執行檔、擁有 main loop 並每幀呼叫你程式碼的 runtime，以及只在開發期出現的工具鏈（編輯器、匯出器）。挑引擎時它很少決定遊戲成敗，真正會改變答案的是目標平台、遊戲是系統驅動還是內容驅動、以及主力是不是 code agent。Bevy 目前只有 runtime 那一半：純 Rust、核心是 ECS，沒有編輯器、UI 年輕、每幾個月一次破壞性改版；用 `Cargo.lock` 鎖版加上 agent 規則檔可以擋掉大部分改版的痛。macOS 上的系統相依只有 Xcode Command Line Tools，因為 `rustc` 不自己做連結，而視窗、繪圖、音訊的 framework 都在系統 SDK 裡。建置那一半往下拆：`cargo run` 是 cargo、rustc、linker 三個 process 接力，rustc 把程式逐層改寫成 HIR、MIR、LLVM IR，再由內建的 LLVM 產生 `.o`；被編譯的單位是 crate（一包原始碼，不是產物），feature 是編譯期的開關，Rust 對 Rust 只發佈原始碼是因為 Rust ABI 沒有凍結，靜態與動態連結差在函式庫的機械碼抄進執行檔還是留在另一個檔案。

## 1. 事件：發生了什麼

這次撞到的不是一行錯誤訊息，是一個做不下去的決定：想獨立做一款遊戲，第一步選引擎就卡住。一路問下來的順序：

```
t0  想獨立做一款遊戲             有哪些引擎、各要多少錢、「Godot 是 PC 端的嗎」     → §3、§4
t1  code agent 都能寫程式了       引擎還有沒有差、是不是選什麼都差不多               → §5
t2  想用 Rust                    Bevy 現在到哪了、「UI 不強」指什麼、能出哪些平台    → §6
t3  Bevy 的核心是 ECS            ECS 是什麼模型                                    → §7
t4  Bevy 每幾個月一次破壞性改版    版本怎麼鎖、agent 寫出舊版 API 怎麼辦              → §8
t5  定第一款                     貪吃蛇：小，但會碰到 Bevy 的每個基本概念            → §9
t6  macOS 從零建環境             排出步驟 0–13 與最小程式；程式碼在 Linux 上
                                 cargo check 過，指令還沒在 Mac 上實跑              → §10
t7  讀到步驟 1 卡住               為什麼要裝 Xcode Command Line Tools、「rustc 不自己做連結」
                                 是什麼意思、為什麼會用到 clang                     → §10 步驟 1
t8  cargo run 背後誰在編譯       cargo、rustc、LLVM、linker 各做什麼；HIR / MIR / LLVM IR 是什麼、
                                 `cargo check` 省掉的是哪一段                       → §11
t9  照 §10 在 Mac 上實跑         第一次建置為什麼這麼久、`--features bevy/dynamic_linking` 撥的是什麼、
                                 crate 是不是 binary、都是機械碼了為什麼還分 Rust ABI 和 C ABI、
                                 靜態和動態連結差在哪                               → §12
```

當時的模型，每一條都會讓選擇走偏（括號是 §15 陷阱清單的編號）：

- 「Godot 是 PC 端的引擎」→ 把編輯器跑在哪、遊戲輸出到哪混成一件事（1）。
- 「引擎選什麼都大差不差」→ 對成敗影響小是真的，但目標平台、遊戲類型、主力是不是 agent 這三個變數會改變答案（2）。
- 「現在都是 code agent 在寫，所以沒什麼難度」→ agent 寫掉的是程式，而程式本來就不是最難的那一塊（3）。
- 「引擎要花很多錢」→ 全部可以免費開始；真正的支出是美術音樂外包和平台抽成（4）。
- 「UI 不強」= 做不出 UI → 是年輕且只能寫程式，做得到但來回慢（5）。
- 「六個平台都支援」= 六個平台一樣好出 →「支援」只表示引擎能在上面跑，不含打包、上架、內購那一段（6）。
- 「用規則檔綁住 agent，版本迭代的問題就解決了」→ 解決大部分；agent 沒學過的新功能、好不好玩，這兩件擋不住（7）。
- 引擎之外順帶被校正的三個：把「端遊」當成手機上的遊戲、把原型當成上架的小產品、以為人手一機所以手機是最好的市場（18–20）。

t7 那一段連續被校正了四次，每一句都是當時說出口的（21–24）：

- 「要裝 Xcode」→ 要裝的是 Command Line Tools，不是那個 IDE（21）。
- 「Rust compile 完還是要跟 C 做 dynamic linker」→ 把建置時的 `ld` 和執行時的 `dyld` 混成一個；出現「C」是因為 C ABI 和 `cc` 這個入口，沒有 C 原始碼被編譯（22）。
- 「透過 cc 做 ABI 串接」→ `cc` 只是啟動 `ld` 的手段；ABI 是 `rustc` 編譯時就遵守的約定，`ld` 只認名字（23）。
- 「裝這包是為了取得 rustc 跟 clang；不裝就是自己配路徑」→ `rustc` 來自 rustup；不裝是東西不存在，繞不過去的是 SDK（24）。

t8 那一段來自 2026-11-16 入庫的另一份草稿，被校正的是 toolchain 上各個角色的分工（25–33）：

- 「Cargo 會把 crate 連接在一起」→ Cargo 只安排順序、路徑、參數；連接是 linker 做的，而 linker 是 rustc 叫起來的（25）。
- 「`cargo check` 只檢查型別」→ 它做完整個前端，包含 borrow check；省掉的是 codegen 與 link（27、28）。
- 「LLVM 是一支獨立的程式，裝在 Xcode Command Line Tools 裡」→ LLVM 是函式庫，隨 rustup 的 toolchain 來，住在 rustc process 裡（29）。
- 「HIR、MIR、LLVM IR 是三種語言或三種檔案」→ 是 rustc 記憶體裡同一支程式的三個改寫版本（33）。

t9 那一段是邊跑邊問的，每一句也都是當時說出口的（34–40）；另外三條是實跑量出來、和原本的推論或說法不一樣的（41–43）：

- 「crate 就是三方套件」「crate 就是一個 binary，compile 出來的 output 都是 crate」→ crate 是 `rustc` 一次編譯的那包原始碼，自己的專案也是一個；編出來的檔案是它的產物（34）。
- 「`--features bevy/dynamic_linking` 是針對這個 crate 做 compile」→ 是撥相依 `bevy` 的一個編譯期開關；開不開，`bevy` 都會被編（35）。
- 「都是機械碼了，為什麼還分 Rust ABI 和 C ABI」→ 指令集是一套，約定是兩套（37）。
- 「動態連結是目的檔透過 linker 封裝成執行檔」→ 那是靜態（38）。
- 實跑撞到的兩個錯誤訊息：`cargo dev` 報 `no such command: dev`（alias 寫進了 `Cargo.toml`，40）、`rustc 1.87.0 is not supported`（`PATH` 上先找到的是 Homebrew 那份，43）。

陷阱 8–17 屬於 ECS 與建置那一側，跟著 §7–§10 收在陷阱清單。

往下挖的順序：先把整條鏈的層次畫出來（§2）→ 引擎是哪兩樣東西（§3）→ 有哪些、各要多少錢（§4）→ 怎麼選（§5）→ Bevy 現在的樣子（§6）→ ECS 的模型（§7）→ 版本怎麼鎖（§8）→ 第一款做什麼（§9）→ macOS 上怎麼從零建起來（§10）→ `cargo run` 背後三個 process 怎麼接力（§11）→ 實跑時問出來的建置名詞（§12）→ 一條指令到第一幀的完整時間軸（§13）→ 回頭把幾條路並排（§14）。

## 2. 分層模型（骨架）

```
層級                 macOS 上是什麼                                    誰寫的          語言 / 怎麼進到你的 process
──────────────────────────────────────────────────────────────────────────────────────────────────────────────
你的遊戲             component、system、resource（snake）               你              Rust，你的 crate
引擎 runtime         bevy_app（main loop、schedule）                    Bevy 專案       Rust crate，靜態連結進執行檔
                     bevy_ecs（World、Query、executor）                                 （開 dynamic_linking 時整包變成
                     bevy_render / bevy_sprite / bevy_ui / bevy_audio                    libbevy_dylib.dylib）
                     bevy_asset / bevy_input / bevy_time / bevy_state
平台抽象             winit（視窗、事件）                                各自的開源專案   Rust crate
                     wgpu + naga（繪圖 API、shader 轉譯）
                     cpal / rodio（音訊）、gilrs（手把）
  ─── Objective-C / C ABI（FFI）──────────────────────────────────────────────────── objc2-* crate
系統 framework       AppKit、Metal、QuartzCore、CoreAudio               Apple           ObjC / C / C++；user mode，由 dyld 載入
  ═══ 使用者／核心邊界：syscall、Mach trap、IOKit ═══════════════════════════════════
kernel               XNU、GPU 的 kernel-mode driver                     Apple
硬體                 CPU、GPU、顯示器、鍵盤

工具鏈（只在開發期，不在執行檔裡）
  rustup → cargo → rustc → cc / ld（Xcode Command Line Tools）          建置
  編輯器（Godot、Unity、Unreal 有；Bevy 還沒有）                          擺場景、調 UI
  匯出器（把同一份專案打包成各平台的執行檔）
```

- 「引擎」和 note 03 的 libtorch 是同一種角色：夾在你的程式和廠商 API 之間，負責排工作和管資源。那邊排的是 tensor 運算，這邊排的是每一幀要跑哪些 system、畫哪些東西。
- 引擎止步於平台抽象層的 API，平台抽象止步於系統 framework，一層藏一層。你的遊戲不知道底下是 Metal、Vulkan 還是 DirectX。
- 整條鏈上面三層全是 Rust，到 `objc2-*` 這層 FFI 才換語言。這也是 macOS 不用另外裝繪圖或音訊函式庫的原因：framework 在系統裡，連結時只需要 SDK。

## 3. 引擎是什麼：runtime + 工具鏈

**核心規則**：引擎是兩樣東西。runtime 是連結進你執行檔的函式庫，擁有 main loop；工具鏈是開發期用的程式，不會跟著遊戲出貨。

**關鍵細節**

- **main loop 屬於引擎，不屬於你。** 你的 `main` 只做一件事：把 system 和 plugin 登記進去，然後呼叫 `.run()` 交出控制權。之後每一幀是 main thread 上引擎的 loop 回頭呼叫你的函式。「誰讓遊戲動起來」的答案是那條跑著 loop 的 main thread，不是你的程式碼。
- **編輯器跑在哪個平台，和遊戲能輸出到哪個平台，是兩個獨立的問題。** Godot 的編輯器跑在 Windows、macOS、Linux，做出來的遊戲可以輸出到這三者加 Android、iOS、網頁。問「Godot 是 PC 端的嗎」是把兩半混成一件事。
- **Bevy 目前只有 runtime 那一半。** 0.19 引入的場景格式 BSN 是替未來的編輯器鋪路，官方編輯器還沒出來，現在是純寫程式的工作流程。

**runtime 裡的子系統**

| 子系統 | 做什麼 | Bevy 的 crate | 往下叫誰（macOS） |
|---|---|---|---|
| 視窗與輸入 | 開視窗、收鍵盤滑鼠事件 | `bevy_window`、`bevy_winit`、`bevy_input` | winit → AppKit |
| 時間 | 每幀的 delta、固定時間步 | `bevy_time` | 系統時鐘 |
| 物件模型 | 遊戲裡的東西怎麼存、邏輯怎麼跑 | `bevy_ecs` | 純記憶體，不往下 |
| 渲染 | 把資料畫成畫面 | `bevy_render`、`bevy_sprite` | wgpu → Metal |
| UI | 選單、文字、按鈕 | `bevy_ui`、`bevy_text` | 同渲染 |
| 音訊 | 播音效與音樂 | `bevy_audio` | rodio → cpal → CoreAudio |
| 資產 | 載入圖片、音檔、場景 | `bevy_asset`、`bevy_scene` | 檔案 I/O |

Godot 的物件模型是「節點與場景」的樹，Bevy 是 ECS，這是兩者寫起來手感差最多的地方。

## 4. 引擎一覽與費用

| 引擎 | 授權與費用 | 語言 | 強項 | 弱項 / 適合誰 |
|---|---|---|---|---|
| Godot | MIT 開源，免費、不抽成 | GDScript（類 Python）、C#；效能關鍵處可用 C++ 或 Rust 寫擴充 | 編輯器輕量、2D 是一等公民、場景檔是純文字 | 3D 不如 Unreal、現成素材與外掛比 Unity 少；主機要靠第三方移植 |
| Unity | 年營收或資金 20 萬美元以下免費；超過要買 Pro，每席每年 2,310 美元 | C# | 手機與跨平台發行最成熟、資料最多 | 很多設定要在編輯器裡手動點 |
| Unreal | 單一產品累計營收前 100 萬美元不抽，超過的部分抽 5% | C++、Blueprint | 高規格 3D | 美術產能需求遠超小團隊 |
| GameMaker | 非商業免費；商業發行買一次性授權，約 100 美元（這個數字沒查到最新來源） | GML | 純 2D、上手最快 | 功能天花板較低 |
| Cocos Creator | 開源免費 | TypeScript | 亞洲手遊與小遊戲平台的生態最強 | 目標是那個市場才選它 |
| Defold | 免費、不抽成 | Lua | 極輕量，適合 2D 手遊與網頁 | 社群小 |
| Bevy | 開源免費 | Rust | ECS、純程式碼、模擬大量實體的效能 | 沒有編輯器、UI 年輕、API 常改；適合興趣與系統驅動的遊戲 |

- 2026 年 GMTK Game Jam 的投稿裡 Godot 佔 47%，超過 Unity 的 34%。
- 對獨立開發來說引擎幾乎不是成本。真正的支出是美術和音樂外包，以及平台的三成抽成。

## 5. 選引擎的判準

**核心規則**：引擎很少是獨立遊戲成敗的瓶頸，選一個成熟的就該往下走；但「選什麼都差不多」只對一半，有三個變數會改變答案。

| 變數 | 怎麼改變答案 |
|---|---|
| 目標平台 | 主攻手機或一開始就要上主機 → Unity；亞洲小遊戲平台 → Cocos Creator；PC 與網頁 → Godot、Bevy 都行 |
| 遊戲類型 | 系統驅動（卡牌構築、經營模擬、自動化）不靠手擺關卡，沒有編輯器的損失小；內容驅動（關卡、劇情）很吃編輯器 |
| 誰是寫程式的主力 | 讓 code agent 當主力時，引擎之間出現新的差別（下表） |

**agent 友善度**

| 引擎 | 對 agent 有利 | 對 agent 不利 |
|---|---|---|
| Godot | 場景與資源檔是純文字，agent 可以讀寫整個專案 | GDScript 訓練資料少，偶爾寫出舊版 3.x 語法 |
| Unity | C# 資料量最大，寫得最穩 | 很多設定要人在編輯器裡點 |
| Unreal | — | Blueprint 是二進位檔，agent 碰不了，只能走 C++ |
| Bevy | 整個專案都是程式碼；API 寫錯會編譯失敗，agent 可以自己修到過 | API 改版頻繁，訓練資料混了很多版本，常寫出過期寫法 |

**關鍵細節**

- agent 寫掉的是程式，而程式本來就不是做遊戲最難的那一塊。它沒碰到的有三件：好不好玩要人實際玩了才知道；美術、音效與整體風格的一致性要人把關；讓人知道這款遊戲存在。
- 判斷 Bevy 適不適合自己的方法是一次有期限的實驗：兩週做完一個極小的遊戲。兩週後大部分時間花在做遊戲就繼續，大部分時間在跟 UI 和工具搏鬥就換 Godot。
- 不想放棄 Rust 的折衷是 godot-rust：遊戲邏輯用 Rust 寫，編輯器和 UI 交給 Godot。
- 以商業上架為目標，Godot 比較穩；Bevy 適合拿來確認自己喜不喜歡做遊戲這件事。

## 6. Bevy 的現況

**核心規則**：Bevy 是用 Rust 寫的開源引擎，核心是 ECS，所有功能都是可拆的 plugin。現在是「年輕、只能寫程式」的狀態，不是「做不到」。

**關鍵細節**

- 版本節奏：2026 年出了 0.18、0.19、0.20 三版，每版都有破壞性變更。
- 所有功能都是 plugin，所以可以關掉畫面只跑邏輯。對數值導向的遊戲，同一份程式碼可以無畫面跑幾萬局來驗證平衡。
- 預設 feature 是四包：`2d`、`3d`、`ui`、`audio`。只做 2D 可以關掉 `3d` 少編一大塊（§10 步驟 6）。

| 適合的地方 | 會痛的地方 |
|---|---|
| ECS 把資料和邏輯分開，系統驅動的遊戲寫起來順 | UI 是弱項，而系統驅動的遊戲大多是選單、卡片、數字面板 |
| 不靠手擺關卡的類型，沒有編輯器的損失小 | 每隔幾個月要照遷移指南改一輪 |
| 整個專案都是程式碼，agent 可以完整讀寫 | agent 常寫出舊版 API |
| 模擬大量實體時效能好 | 編譯慢，拖累「改一個數字馬上看結果」的節奏 |
| | 手機與主機的輸出不如 Godot、Unity 成熟 |

**「UI 不強」的具體意思**

1. 沒有視覺化編輯器。每個選單用程式碼排版（類似 CSS 的 flexbox），調一個間距要改程式、重編、再看結果。
2. 元件很新。下拉選單、可捲動清單、分頁這些是 0.20 才加入的；有外觀的那套 Feathers 是為編輯器工具設計的，遊戲要用得自己做樣式。
3. UI 動畫與轉場通常要靠第三方套件或手寫。

卡牌遊戲的卡片本身通常是場景裡的 sprite 而不是 UI，那部分 Bevy 處理得很好。

**平台支援**

| 平台 | 狀態 |
|---|---|
| Windows、Linux、macOS | 完整支援，主要開發目標 |
| Web | 編成 WebAssembly；可另外啟用 WebGPU，啟用後不支援 WebGPU 的瀏覽器就跑不了 |
| iOS、Android | 官方有範例與建置說明，沒有一鍵輸出；廣告、內購這類商店 SDK 沒有官方整合 |
| 主機 | 沒有官方支援 |

## 7. ECS：一個每幀被查詢和更新的資料庫

**核心規則**：Entity 只是一個 ID；Component 是掛在 entity 上的純資料；System 是一個函式，每幀對「擁有某幾種 component 的所有 entity」做事。

**用後端的角度看**

| ECS | 資料庫 | 貪吃蛇裡 |
|---|---|---|
| Entity | 主鍵 | 蛇頭、每一節蛇身、食物各一個 |
| Component | 欄位 | `Position`、`SnakeHead`、`Food` |
| System | 每幀執行的「查詢 + 更新」 | 移動、吃食物、判斷撞到自己 |
| Resource | 全域唯一的一列設定 | 分數、目前方向、蛇身順序 |
| `Query<(&mut A, &B)>` | `SELECT ... WHERE 同時有 A 和 B` | 找出所有有位置的蛇身 |

| 實體 | Position | Velocity | Health | Poisoned |
|---|---|---|---|---|
| 1（玩家） | ✓ | ✓ | ✓ | |
| 2（敵人） | ✓ | ✓ | ✓ | ✓ |
| 3（石頭） | ✓ | | | |

「移動 system」查詢同時有 Position 和 Velocity 的 entity，所以處理 1 和 2、跳過 3。「中毒 system」只處理 2。system 不在乎對象是玩家還是敵人，只看它有哪些 component。

**和物件導向的差別**：傳統寫法是 `Player` 繼承 `Character` 繼承 `GameObject`，資料和方法綁在類別裡，需求一交叉就卡住（「會飛、會中毒、可被拾取的箱子」該繼承誰）。ECS 裡想讓石頭會動就掛一個 `Velocity`，行為由 component 的組合決定，執行期間可以隨時增減。

| 好處 | 代價 |
|---|---|
| 組合彈性：新機制通常只是加一個 component 和一個 system | 邏輯拆散在很多 system，沒有單一地方能看「這個東西會發生什麼事」 |
| 效能：同種 component 在記憶體裡連續存放，對 CPU cache 友善 | system 之間的執行順序要自己留意 |
| 引擎知道每個 system 讀寫什麼，可以自動平行執行 | 一次性的劇情邏輯（打倒這隻王後開那扇門）寫起來比物件導向迂迴 |

**誰在跑 system（主詞要精確）**

- system 是普通函式，自己不會跑。跑它的是 `bevy_ecs` 裡的 schedule executor：一段由 main thread 在每次 `app.update()` 時啟動的程式碼。
- executor 讀每個 system 的參數型別就知道它要什麼：`Query<&mut Transform>` 是寫 `Transform`，`Res<Time>` 是讀 `Time`。讀寫不衝突的 system 被丟到 compute task pool 的 worker thread 上同時跑，衝突的排先後。
- 這就是 note 04 的「多讀 XOR 單寫」，只是檢查從編譯期的 borrow checker 搬到執行期的排程：同一份資料可以多個 system 同時讀，或一個 system 獨佔寫。

**Bevy 詞彙**

| 詞 | 是什麼 |
|---|---|
| `App` | 登記簿：收集 plugin、system、resource，最後 `.run()` |
| Plugin | 一包登記動作；`DefaultPlugins` 是視窗、輸入、渲染、時間等幾十個 plugin 的集合 |
| Schedule | system 的分組與執行時機：`Startup` 開場一次、`Update` 每幀、`FixedUpdate` 固定時間步（預設 64 Hz） |
| `Commands` | 延後執行的結構性變更。`commands.spawn(...)` 只是排隊，到同步點才真的寫進 World |
| Resource | 全域唯一的資料，用 `Res<T>` / `ResMut<T>` 取 |
| State | 遊戲狀態機（遊戲中、結束、選單） |
| Message | system 之間傳遞的佇列訊息，例如鍵盤輸入。舊教材和舊版叫 Event，是 agent 最常寫錯的地方之一 |

每幀的 schedule 順序（0.20.1 原始碼）：`First → PreUpdate → StateTransition → RunFixedMainLoop → Update → SpawnScene → PostUpdate → Last`。`RunFixedMainLoop` 會依經過的時間把 `FixedUpdate` 跑零到多次。渲染不在這條主 schedule 裡，在另一個 sub-app、另一條 thread 上（§13）。

## 8. 版本鎖定與 agent 規則檔

**核心規則**：`Cargo.toml` 寫的是「可接受的範圍」，`Cargo.lock` 記的是「實際用的那一版」。鎖住版本的是後者。

| 檔案 | 內容 | 誰改它 |
|---|---|---|
| `Cargo.toml` | `bevy = "0.20"`，意思是 0.20.x 都接受 | 你，或 `cargo add` |
| `Cargo.lock` | `bevy 0.20.1` 以及全部間接相依的確切版本 | cargo 在解析相依時寫入；之後只有 `cargo update` 會動它 |

**關鍵細節**

- `Cargo.lock` 要 commit 進 git。少了它，換一台機器或過一陣子重新解析，可能拿到不同的 patch 版。
- 規則檔寫四條：只用 `Cargo.lock` 裡的那個 Bevy 版本；用任何 API 前先查本機的原始碼和官方 examples；每次改動後跑 `cargo check`；不准跑 `cargo update`。
- 鎖版加規則檔加編譯器把關，能解決大部分改版問題。Rust 的優勢是 API 寫錯會直接編譯失敗，agent 可以自己修到過。

| 這套擋得住 | 擋不住 |
|---|---|
| 舊版 API、改名的型別、搬家的模組（編譯失敗） | 新功能 agent 沒學過（例如 0.20 又改了一輪的 BSN 語法），只能靠你餵的文件 |
| 相依版本自己漂移 | 編譯器只驗證「能編過」，UI 好不好看、手感對不對要人自己看 |

## 9. 第一款：貪吃蛇會碰到的概念

**核心規則**：貪吃蛇大約兩三百行，小歸小，會碰到 Bevy 的每個基本概念。不要讓 agent 一次生成整個遊戲；自己一個 system 一個 system 寫，讓 agent 負責解釋和除錯。

| 概念 | 貪吃蛇裡是什麼 | Bevy 機制 |
|---|---|---|
| component 與生成 | 蛇頭、蛇身、食物是帶不同 component 的 entity | `#[derive(Component)]`、`commands.spawn` |
| resource | 分數、目前方向 | `Res` / `ResMut` |
| 固定時間步 | 蛇每 0.15 秒走一格，不是每幀移動 | `FixedUpdate` 配 `Time::<Fixed>::from_seconds`，或 `on_timer` 條件 |
| 輸入 | 鍵盤每幀都讀，移動只在時間到才發生 | 讀輸入的 system 放 `Update`，移動的放 `FixedUpdate` |
| 生成與銷毀 | 吃到食物加一節、食物消失再隨機生成 | `commands.spawn` / `despawn` |
| 狀態 | 遊戲中、結束、重新開始 | State |

**兩個會卡住的地方**

1. 蛇身有順序，而 ECS 的 query 不保證順序。常見做法是用一個 resource 存 `Vec<Entity>` 記錄身體的排列。
2. 輸入和移動分屬不同 system。不做緩衝的話，在兩次移動之間快速連按兩個方向，蛇會直接回頭撞自己。

基本版之後加分數顯示、結束畫面、開始選單、音效，這幾步會刻意撞上 UI，做完就能親自判斷 Bevy 適不適合拿來做正式專案。

## 10. macOS 環境建置：每個環節

**核心規則**：macOS 上唯一的系統相依是 Xcode Command Line Tools。`rustc` 只把每個 crate 編成目的檔，最後一步連結是呼叫 `cc`，再由 `cc` 呼叫 Apple 的 `ld`；視窗、繪圖、音訊的 framework 都在系統 SDK 裡，不像 Linux 要另外裝 alsa、udev 的開發套件。

### 總覽

| # | 環節 | 指令 | 裝了什麼 / 產生什麼 | 屬於 §2 的哪一層 |
|---|---|---|---|---|
| 0 | 確認機器 | `uname -m` | 知道 target triple | 硬體 |
| 1 | Xcode Command Line Tools | `xcode-select --install` | clang（當 `cc`）、`ld`、macOS SDK、git | 工具鏈：連結 |
| 2 | rustup | rustup.rs 的安裝指令 | `~/.rustup`（toolchain 本體）、`~/.cargo/bin`（proxy） | 工具鏈：管理 |
| 3 | 更新到最新 stable | `rustup update stable` | 最新的 rustc、cargo、std | 工具鏈：編譯 |
| 4 | 編輯器 | VS Code + rust-analyzer | 型別提示、即時錯誤 | 工具鏈（非必要） |
| 5 | 建專案 | `cargo new snake` | `Cargo.toml`、`src/main.rs`、`.git`、`.gitignore` | 你的遊戲 |
| 6 | 加 Bevy | `cargo add bevy@0.20` | `Cargo.toml` 多一行相依 | 引擎 runtime |
| 7 | dev profile | 手改 `Cargo.toml` | 相依全速最佳化、自己的 crate 輕度 | 工具鏈：編譯設定 |
| 8 | 最小程式 | 改 `src/main.rs` | 開一個空白視窗 | 你的遊戲 |
| 9 | 第一次建置與執行 | `cargo run --features bevy/dynamic_linking` | `target/`、`Cargo.lock` | 全部 |
| 10 | 進版控 | `git commit` | `Cargo.lock` 入庫 | — |
| 11 | 參考原始碼 | 讀 registry 或 clone tag | 對應版本的 examples | — |
| 12 | agent 規則檔 | 專案根目錄一份 | §8 的四條 | — |
| 13 | 日常迴圈 | `cargo check`、`cargo dev` | — | — |

### 步驟 0：確認機器

```bash
uname -m     # arm64 = Apple Silicon；x86_64 = Intel
sw_vers      # macOS 版本
```

決定 target triple：Apple Silicon 是 `aarch64-apple-darwin`，Intel 是 `x86_64-apple-darwin`（經驗 01 §5 的五層檢查）。rustup 會自己偵測，這步只是讓你看得懂之後 `rustup show` 印的東西。

### 步驟 1：Xcode Command Line Tools

```bash
xcode-select --install     # 跳出系統對話框，按「安裝」；已裝過會顯示 already installed
```

驗證：

```bash
xcode-select -p            # 印出 /Library/Developer/CommandLineTools（裝了完整 Xcode 則是 Xcode.app 內的路徑）
cc --version               # Apple clang
xcrun --show-sdk-path      # macOS SDK 的位置
```

- 不需要完整的 Xcode，Command Line Tools 就夠。
- SDK 裡放的是 framework 的 header 和連結用的 stub。連結時 `ld` 靠它知道「AppKit 有這些符號」，真正的 framework 本體在系統裡，執行時由 dyld 載入。

#### 為什麼要裝它：編譯和連結是兩個步驟

**核心規則**：把原始碼變成執行檔有兩步，`rustc` 只親手做第一步。連結需要的三樣東西（操作員 `cc`、機器 `ld`、零件目錄 macOS SDK）都是 Apple 的，只透過這包發放；`rustc` 是另外由 rustup 裝的，它只是這三樣東西的使用者。

| 步驟 | 做什麼 | 誰做 | 產物 |
|---|---|---|---|
| 編譯 compile | 把一個 crate 的原始碼翻成機械碼 | `rustc` 自己 | 目的檔（`.o` / `.rlib`） |
| 連結 link | 把很多份目的檔接成一個能執行的檔案 | `ld`（`rustc` 透過 `cc` 叫它） | Mach-O 執行檔 |

目的檔是半成品。`rustc` 一次只看一個 crate，不知道別的 crate 和系統函式庫最後擺在哪，呼叫 `write` 的地方只能先留空格，附一張「我缺 `write` 的位址」的清單。`ld` 把所有目的檔擺在一起、決定最終位置、把空格填上（或記下「執行時去 libSystem 找」）。

```
t0  rustc          把你的 crate 編成目的檔；呼叫系統函式的機械碼在這一刻就照 C ABI 擺好參數
t1  rustc → cc     「幫我把這些接成執行檔」            ← 沒裝：linker 'cc' not found
t2  cc → ld        cc 補上 SDK 路徑、最低 macOS 版本、架構、libSystem、啟動碼，再叫 ld
                                                     ← 沒裝：xcrun: invalid active developer path
t3  ld             照符號名字把目的檔接起來；對系統函式庫只記「需要 libSystem 的 write」
t4  （執行時）dyld  載入 libSystem 與 framework 本體，填上那筆空格   ← 這時已經不需要 SDK
```

- **clang 在這裡一行 C 都沒編。** 它收到 `.c` 才當編譯器；收到的全是目的檔時只做一件事：用對的參數啟動 `ld`。`rustc` 要的是「它知道怎麼叫 `ld`」，那串參數每個平台、每個 OS 版本都不同。
- **ABI 是約定，不是動作，沒有人在「串 ABI」。** 遵守 C ABI 是 `rustc` 在 t0 產生機械碼時的事；`ld` 在 t3 只認符號名字，不檢查兩邊擺法對不對。
- **`ld` 和 `dyld` 是兩個時間的兩個程式。**

| | linker（`ld`） | dynamic linker（`dyld`） |
|---|---|---|
| 什麼時候跑 | 建置時的最後一步 | 每次執行程式時 |
| 做什麼 | 把目的檔組成執行檔；對系統函式庫只記名字 | 把函式庫載進記憶體，填上真正位址 |
| 誰提供 | Command Line Tools | macOS 內建 |

- **哪些是靜態、哪些是動態。** Rust crate 之間（你的程式、std、bevy）建置時由 `ld` 靜態接在一起，全進執行檔；只有碰到作業系統那一層（libSystem、AppKit、Metal）才是動態的，那一層講的是 C ABI。macOS 規定只能透過 libSystem 進 kernel、不提供靜態版，所以任何語言的程式都動態連結它；Linux 承諾 syscall ABI 穩定，才做得出完全靜態的 musl 執行檔（經驗 01 §6、§9）。
- **不是 Bevy 才需要。** `Hello, world!` 也要連結 libSystem，步驟 5 的第一次 `cargo run` 就會用到。
- **不裝的話，問題是東西不存在，不是路徑沒設好。** 全新的 Mac 上 `/usr/bin/cc` 只是空殼，執行它會跳出安裝對話框；`ld` 和 SDK 根本沒有。

| 不透過這包的做法 | 可不可行 |
|---|---|
| 裝完整 Xcode | 可以，裡面是同一套東西，只是大很多 |
| 自己裝別的 linker（LLVM 的 lld） | linker 有了，SDK 還是沒有，照樣連不起來（推論） |
| 完全不裝 Apple 的東西 | 不行，卡在 SDK |

繞不過去的是 SDK，不是 clang。能「設路徑」的是反過來的情況：同一套東西裝了不只一份（Command Line Tools 加完整 Xcode），用 `xcode-select -s` 選用哪一份。

### 步驟 2：安裝 rustup

先看有沒有裝過：

```bash
command -v rustup && rustup --version
```

沒有的話用官網 rustup.rs 給的指令：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh    # 選 1) Proceed with standard installation
. "$HOME/.cargo/env"                                              # 或直接開一個新的 terminal
```

驗證：

```bash
rustup show          # default host 與 active toolchain
rustc -vV            # host: aarch64-apple-darwin
cargo --version
which -a rustc       # 應該只有 ~/.cargo/bin/rustc
```

- 裝到哪：`~/.rustup/toolchains/stable-aarch64-apple-darwin/` 是 rustc、cargo、std 的本體；`~/.cargo/bin/` 放的是 rustup 的 proxy，你打 `cargo` 時先跑到 proxy，它再依目前的 toolchain 設定轉呼叫真正的 cargo。
- `which -a rustc` 若還列出 `/opt/homebrew/bin/rustc`，表示 Homebrew 也裝了一份，rustup 管不到那一份。確認 `~/.cargo/bin` 在 PATH 前面，或把 Homebrew 那份移除。

### 步驟 3：更新到最新 stable（必做）

```bash
rustup update stable
rustc --version
```

Bevy 的最低支援 Rust 版本（MSRV）政策是「最新的 stable」。實測：bevy 0.20.1 的 `rust-version` 是 1.97.1，拿 rustc 1.97.0 去編會直接被 cargo 拒絕，連 patch 版都卡。

### 步驟 4：編輯器

裝 VS Code，在 Extensions 面板搜尋 rust-analyzer（ID 是 `rust-lang.rust-analyzer`）安裝。

- rust-analyzer 是另一個 process，在背景對你的專案跑 `cargo check`，把型別和錯誤顯示在編輯器裡。行內型別提示在學 Rust 階段特別有用。
- 它不是建置的必要條件。第一次打開 Bevy 專案時它也要把整個相依檢查一遍，風扇會轉一陣子，這是正常的。

### 步驟 5：建立專案

```bash
mkdir -p ~/work && cd ~/work
cargo new snake && cd snake
cargo run                    # 檢查點：應該印出 Hello, world!
```

- `cargo new` 產生 `Cargo.toml`、`src/main.rs`、一個新的 git repo 和內容為 `/target` 的 `.gitignore`。
- 專案放在筆記 repo 外面。`target/` 會長到好幾 GB，而且在既有的 git repo 裡 `cargo new` 不會另外開 repo。
- 先在這裡跑一次 `cargo run`。這個檢查點把「toolchain 和 linker 有沒有裝好」跟「Bevy 編不編得過」分開，出錯時知道是哪一段。

### 步驟 6：加入 Bevy

```bash
cargo add bevy@0.20
```

`Cargo.toml` 的 `[dependencies]` 會多一行 `bevy = "0.20"`，實際解析到 0.20.1。

可選：只做 2D 的話關掉 3D 那包，少編一大塊。這個寫法 `cargo check` 驗證過：

```toml
bevy = { version = "0.20", default-features = false, features = ["2d", "ui", "audio"] }
```

### 步驟 7：dev profile

在 `Cargo.toml` 尾端加上：

```toml
# 自己的 crate：稍微最佳化，保留快速重編
[profile.dev]
opt-level = 1

# 所有相依（Bevy 本體）：全速最佳化，只編一次
[profile.dev.package."*"]
opt-level = 3
```

不加的話 debug build 的 Bevy 會慢到不能用。相依以 crate 為單位快取，全速最佳化只拖慢第一次建置，之後改自己的程式不會重編它們。

完成後 `Cargo.toml` 長這樣：

```toml
[package]
name = "snake"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = "0.20"

[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

### 步驟 8：最小程式

`src/main.rs`：

```rust
use bevy::prelude::*;

fn main() {
    // DefaultPlugins：視窗、輸入、渲染、時間…全部在這包裡
    App::new().add_plugins(DefaultPlugins).run();
}
```

### 步驟 9：第一次建置與執行

```bash
cargo run --features bevy/dynamic_linking
```

預期看到：

- 第一次要編幾分鐘（`Cargo.lock` 裡約 570 個 crate，macOS 實際編其中一部分），之後改 `main.rs` 重編只要幾秒。
- 跳出一個空白視窗，標題是執行檔的名字 `snake`（Bevy 預設拿執行檔名當標題）。
- terminal 印出幾行 `INFO`，其中一行是 `AdapterInfo { ... }`。這行是 `bevy_render` 印的，Mac 上 backend 應該是 Metal，可以拿來對照 §2 的分層（推論：實際內容沒在 Mac 上看過）。
- 關掉視窗或按 Cmd+Q，`run()` 才 return，process 結束。

`dynamic_linking` 做的事：把整個 Bevy 編成一個 `libbevy_dylib.dylib`，你的執行檔只記「我需要這個 dylib」。之後改自己的程式，連結步驟不用再把整包 Bevy 靜態連進來，這是迭代重編最大的加速。

| 加速手段 | macOS 上要不要做 |
|---|---|
| `dynamic_linking` | 要，只在開發時用，發佈時拿掉 |
| 換 linker（lld、mold） | 不用，官方說 macOS 預設的 linker 已經比 LLD 快 |
| Cranelift backend | 不要，官方註明 macOS 上 Bevy app 可能 crash |

### 步驟 10：進版控

```bash
git add -A
git commit -m "Init snake with bevy 0.20"
git status --short      # target/ 不應該出現；Cargo.lock 應該已入庫
```

### 步驟 11：對應版本的參考原始碼

兩個來源，都要對齊 `Cargo.lock` 裡的版本：

```bash
# A. 已經在本機：cargo 下載的 crate 原始碼，含 examples 資料夾，只能讀
ls ~/.cargo/registry/src/index.crates.io-*/bevy-0.20.1/examples

# B. 想實際跑範例：clone 同一個 tag（v0.20.1 這個 tag 確認存在）
git clone --branch v0.20.1 --depth 1 https://github.com/bevyengine/bevy ~/work/bevy-src
cd ~/work/bevy-src && cargo run --example breakout     # 會另外建一份好幾 GB 的 target/
```

`examples/` 是最可靠的寫法來源，自己查和寫進 agent 規則檔都指向這裡。

### 步驟 12：agent 規則檔

在專案根目錄放一份規則檔（用 Claude Code 就是 `CLAUDE.md`），寫 §8 的四條，並把步驟 11 的路徑寫進去，讓 agent 知道去哪裡查。

### 步驟 13：日常迴圈與磁碟

把常用的執行方式設成 alias，新增 `.cargo/config.toml`：

```toml
[alias]
dev = "run --features bevy/dynamic_linking"
```

| 想做什麼 | 指令 |
|---|---|
| 只檢查型別，最快 | `cargo check` |
| 開發時跑起來 | `cargo dev` |
| 出一個可以獨立執行的檔案 | `cargo build --release`（不帶 `dynamic_linking`），產物在 `target/release/snake` |
| 看 `target/` 多大 | `du -sh target` |
| 清掉重來 | `cargo clean` |

### 觀察用範例

視窗跑得出來之後，把 `src/main.rs` 換成這段，可以同時看到 component、system、resource 和「system 被哪條 thread 執行」：

```rust
use bevy::prelude::*;

// Component（元件）：純資料，貼在 entity 上
#[derive(Component)]
struct Speed(f32);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup) // 開場跑一次
        .add_systems(Update, (move_square, report)) // 每一幀都跑
        .run(); // 主迴圈從這裡開始，關視窗才會 return
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d); // 沒有攝影機就什麼都畫不出來
    commands.spawn((
        Sprite::from_color(Color::srgb(0.9, 0.4, 0.1), Vec2::splat(80.0)),
        Transform::default(),
        Speed(2.0),
    ));
}

// System（系統）：一個普通函式，參數宣告「我要哪些資料」
fn move_square(time: Res<Time>, mut query: Query<(&mut Transform, &Speed)>) {
    for (mut transform, speed) in &mut query {
        transform.translation.x = (time.elapsed_secs() * speed.0).sin() * 300.0;
    }
}

// 每 60 幀印一次：觀察這個 system 被哪條 thread 執行
fn report(mut frame: Local<u32>, time: Res<Time>) {
    *frame += 1;
    if *frame % 60 == 0 {
        println!(
            "frame {:>5} | dt {:>6.2} ms | {:?}",
            *frame,
            time.delta_secs() * 1000.0,
            std::thread::current().id()
        );
    }
}
```

預期：一個橘色方塊左右擺動，terminal 每 60 幀印一行 frame、dt、ThreadId。

### 排錯

| 症狀 | 原因 | 處理 |
|---|---|---|
| `xcrun: error: invalid active developer path` | Command Line Tools 不在，常見於 macOS 大版本升級之後 | 重跑 `xcode-select --install` |
| `error: linker 'cc' not found` | 同上 | 同上 |
| `command not found: cargo` | PATH 還沒載入 `~/.cargo/env` | 開新的 terminal |
| `rustc 1.97.0 is not supported by the following packages: bevy@0.20.1 requires rustc 1.97.1` | toolchain 比 Bevy 的 MSRV 舊 | `rustup update stable` |
| `rustup update` 後版本沒變 | PATH 上先找到的是 Homebrew 的 rustc | `which -a rustc` 檢查順序 |
| 直接執行 `./target/debug/snake` 報 dyld 找不到 `libbevy_dylib`（推論） | 帶 `dynamic_linking` 編的執行檔要靠 `cargo run` 設的 `DYLD_FALLBACK_LIBRARY_PATH` 才找得到 dylib | 用 `cargo run`，或不帶這個 feature 重編 |
| 改一行重編還是很久 | 沒帶 `dynamic_linking`，或 profile 沒設 | 檢查步驟 7、9 |
| 視窗跳不出來 | macOS 沒有可以單獨更新的顯卡驅動，Metal driver 跟著 OS 走 | 看 terminal 的錯誤訊息，不是去更新驅動 |

> **補強**（2026-11-16 實測，入庫時加，非這條線的導師內容）：上表「直接執行 `./target/debug/snake`」那一列，在 Mac 上跑出來的訊息是 `dyld: Library not loaded: @rpath/libstd-<hash>.dylib`、`Reason: no LC_RPATH's found`。找不到的是 Rust 的 `libstd`，不是 `libbevy_dylib`；原因與處理不變，細節在 §12.5。`rustc … is not supported` 與「PATH 上先找到 Homebrew 的 rustc」這兩列這次都實際撞到了（§12.6）。

## 11. Rust toolchain 與編譯流程：從 `cargo run` 到執行檔

`cargo run` 不是一個程式在編譯，而是三個 process 接力：cargo 負責安排，rustc 把 Rust 逐層改寫成越來越簡單的 IR、再交給內建的 LLVM 產生 `.o`，linker 把所有 `.o` 排好位址、填好洞、寫成執行檔。

### 11.1 分層模型（骨架）

```
shell
 └ cargo process       安排：依賴圖、下載、編譯順序、rustc 的參數
    └ rustc process    前端（AST → HIR → MIR）+ LLVM（函式庫，住在 rustc 內）
       └ linker process   cc → ld：.o + .rlib + std + 系統庫 → 執行檔
```

每一段都是上一個 process fork+exec 出下一個，沒有一個中央角色在「編譯整個專案」。

誰提供什麼（macOS）：

| 東西 | 誰裝的 | 位置 |
|---|---|---|
| rustc、cargo、std、clippy、rustfmt | rustup | `~/.rustup/toolchains/` |
| rustc 用的 LLVM | rustup，包在 rustc 裡 | 同上的 `lib/` |
| cc、ld（linker） | Xcode Command Line Tools | `/Library/Developer/CommandLineTools/` |
| libc（libSystem） | macOS | 系統內建 |

### 11.2 Toolchain 分工

**核心規則**：只需要安裝 rustup，其他都是它帶進來的。Cargo 是「套件管理 + build tool」，真正的 compiler 是 rustc，真正做連結的是 linker。

| 工具 | 角色 | 其他生態系的對應 |
|---|---|---|
| rustup | toolchain 安裝與版本管理 | nvm / pyenv |
| cargo | 讀 `Cargo.toml`、抓依賴、排順序、呼叫 rustc | npm + make |
| rustc | 把**一個** crate 編成機器碼 | gcc / tsc |
| crates.io | 套件倉庫 | npm registry |

**關鍵細節**

- `~/.cargo/bin/` 裡的 `cargo`、`rustc` 是 rustup 的 proxy：先查目前該用哪個 toolchain，再 exec 真正的那一支。
- rustc 一次只編一個 crate，它不知道其他 crate 在哪。不用 Cargo 就要自己按順序編，並手動給路徑：

```bash
rustc --crate-type lib --crate-name util util.rs    # 產出 libutil.rlib
rustc main.rs --extern util=libutil.rlib            # 手動指出 util 在哪
```

- `cargo build -v` 會印出 Cargo 實際下的每一條 rustc 指令。
- rustc 自己不做最後的 link，所以機器上要有系統 linker。

### 11.3 編譯管線：多層 IR

**核心規則**：IR（Intermediate Representation，中間表示）是「同一支程式的另一種寫法」。rustc 把程式逐層改寫，每往下一層就丟掉一些資訊、形式更簡單；每種檢查都放在最適合它的那個形狀上做。

| 階段 | 產物 | 做什麼 |
|---|---|---|
| parse | AST | 文字變語法樹 |
| macro 展開 + 名稱解析 | AST | 展開 macro、每個名字指向誰 |
| lowering | HIR | 去語法糖：`for` 變 `loop + match`、`?` 變 `match` |
| 型別檢查 | HIR | 推導型別、解 trait |
| lowering | MIR | 樹變成控制流程圖（區塊 + 跳轉） |
| borrow check | MIR | 沿每條執行路徑檢查借用 |
| monomorphization | MIR | 泛型展開成具體型別，各一份 |
| codegen | LLVM IR | 交給 LLVM |

同一行程式走過三層：

```rust
// 原始碼
let hp = s.parse::<u32>()?;
```

```
// HIR：還是樹，? 被攤開成 match（簡化版）
let hp = match s.parse::<u32>() {
    Ok(v)  => v,
    Err(e) => return Err(e),
};

// MIR：流程圖，一塊只做一件小事
bb0: r = parse(s)            -> bb1
bb1: switch r 的標籤  [Ok -> bb2, Err -> bb3]
bb2: hp = r 裡的值;  goto bb4
bb3: 回傳值 = Err(..); return
bb4: （繼續往下）

// LLVM IR：Result 不存在了，只剩位元組與跳轉
%tag = load i8, ptr %r
%err = icmp eq i8 %tag, 1
br i1 %err, label %bb3, label %bb2
```

| | HIR | MIR | LLVM IR |
|---|---|---|---|
| 全名 | High-level IR | Mid-level IR | LLVM 的 IR |
| 誰定義 | Rust 團隊 | Rust 團隊 | LLVM 專案 |
| 形狀 | 樹 | 流程圖 | 流程圖 |
| 誰產生 | rustc，從 AST | rustc，從 HIR | rustc 的 codegen，從 MIR |
| 誰使用 | 型別檢查 | borrow check | LLVM 最佳化、產生 `.o` |
| 還有泛型 `T` | 有 | 有 | 沒有 |
| 還有借用、lifetime | 有 | 有 | 沒有，只剩指標 |

**為什麼要分層**

- 型別檢查要樹：保留你寫的結構，錯誤訊息才能指回原始碼的位置；先去糖是為了讓檢查器要處理的寫法變少。
- borrow check 要流程圖：它問的是「沿每一條可能的執行路徑，這個借用還活著嗎」，樹看不出路徑。
- 產生機器碼要與語言無關：LLVM 服務很多語言，它的 IR 不能帶任何 Rust 概念。

**關鍵細節**：HIR、MIR、LLVM IR 都是 rustc process 記憶體裡的資料結構，不是檔案。要看得用：

```bash
rustc --emit=mir main.rs       # main.mir
rustc --emit=llvm-ir main.rs   # main.ll
rustc --emit=asm main.rs       # main.s
rustc --emit=obj main.rs       # main.o
```

### 11.4 `cargo check`：把前端單獨切出來

**核心規則**：check 做完**整個前端**（macro 展開、名稱解析、型別檢查、borrow check），停在 codegen 之前。

```
parse → macro 展開 → 名稱解析 → 型別檢查 → borrow check
─────── cargo check 停在這，輸出 .rmeta ───────
monomorphization → LLVM IR → 最佳化 → .o → linker → 執行檔
```

**關鍵細節**

- check 有產出：`.rmeta`（Rust metadata，只有型別與簽名、沒有機器碼）。下游 crate 靠它就能繼續 check。
- 編譯時間的大頭在虛線以下（LLVM 最佳化與 link），而寫程式時要的回饋（型別錯、借用錯）全在虛線以上。

### 11.5 LLVM：把 N × M 變成 N + M

**核心規則**：LLVM 是獨立的編譯器後端專案，定義了一個與語言無關的中間格式 LLVM IR。各語言只要翻到 LLVM IR，各 CPU 只要從 LLVM IR 翻出去。

```
沒有共同 IR：5 種語言 × 4 種 CPU = 20 個翻譯器，最佳化各寫各的

C     ─┐             ┌─ x86-64
C++   ─┤             ├─ arm64
Rust  ─┼─ LLVM IR ───┼─ RISC-V
Swift ─┤  (最佳化)    └─ WASM
Zig   ─┘
有共同 IR：5 個前端 + 4 個後端 = 9 個
```

| 情境 | 要做的事 | 誰受益 |
|---|---|---|
| 出現新 CPU | 寫一個後端 | 所有語言 |
| 出現新語言 | 寫一個前端 | 立刻能跑在所有 CPU |
| 新的最佳化 | 在 LLVM IR 上寫一次 | 所有語言 |

**關鍵細節**

- rustc 就是「Rust 前端」，終點是 LLVM IR。
- LLVM 在 rustc 裡是**函式庫**：rustc 在記憶體裡直接呼叫 LLVM 的 API 把 IR 建出來。跑 LLVM 程式碼的是 rustc 的 thread。
- LLVM 做兩件事：最佳化 IR（inline、常數折疊、刪除死碼、向量化），然後選指令、分配暫存器、寫出 `.o`。
- LLVM IR 像一台「假想機器」的組語（暫存器無限多），那台機器不存在，只是各種真實 CPU 的共同起點。
- `rustc -vV` 最後一行是內建的 LLVM 版本；`clang --version` 是 Xcode 那一份，兩者無關。

### 11.6 object file 與 link

**核心規則**：`.o` 是還不能跑的半成品，因為呼叫外部函式的位址只能先留洞。linker 的工作是把所有 `.o` 排好位址，再回頭把洞填上。

`.o` 裡的四樣東西：

| 區塊 | 內容 |
|---|---|
| `.text` | 機器碼 |
| `.data` / `.rodata` | 全域資料、字串常數 |
| symbol table | 我定義了誰 / 我需要誰（未定義） |
| relocation | 哪些位置的位址還是洞、要填誰 |

linker 四步：

| 步驟 | 做什麼 | 失敗長怎樣 |
|---|---|---|
| 收集輸入 | 你的 `.o`、依賴的 `.rlib`、std、系統庫 | |
| 符號解析 | 每個「我需要誰」配到一個「我定義了誰」 | `undefined symbol` / `duplicate symbol` |
| 排位址 | 同類區塊合併，函式與變數拿到最終位址 | |
| relocation | 照表填洞，寫出 Mach-O（macOS）/ ELF（Linux） | |

**關鍵細節**

- `.rlib` = 一包 `.o` 加 metadata。library crate 只產出 `.rlib`，不 link；只有最終的執行檔才 link。
- 比喻：每個 `.o` 是一章書稿，寫著「見第 ? 頁」；linker 排好章節、定出頁碼，再把每個「?」填成真頁碼。
- `nm main.o` 可以看 symbol table，`U` 開頭的就是未定義、等 linker 填。

> **補強**（入庫時加，非這條線的導師內容）：「library crate 只產出 `.rlib`，不 link」說的是預設的 `crate-type = rlib`。`dylib` 與 `cdylib` 兩種 library crate 會經過 linker 產出 `.dylib`；開了 `dynamic_linking` 的 `bevy_dylib` 就是這一種（§12.5、§13 t3）。

| | 靜態連結 | 動態連結 |
|---|---|---|
| Rust 的預設對象 | Rust crate、std | 系統 libc |
| 程式碼在哪 | 抄進執行檔 | 留在外部函式庫 |
| 位址何時定 | link 時 | 程式啟動時 |

### 11.7 可遷移的工程模式

| 模式 | 這條線上的例子 |
|---|---|
| 版本管理器 + proxy | rustup 與 `~/.cargo/bin/` 的 proxy |
| 安排者與執行者分離 | cargo 只下指令，rustc 才編譯 |
| 逐層 lowering | AST → HIR → MIR → LLVM IR，每層只做適合它的檢查 |
| 共同中間格式 | LLVM IR 把 N × M 變 N + M |
| 介面與實作分離 | `.rmeta`（簽名）與 `.o`（機器碼） |
| 快路徑給快速回饋 | `cargo check` 只跑前端 |
| 延後綁定 | 位址先留洞，link 時或啟動時才填 |

### 11.8 思維鏈：`cargo run`（hello 依賴 util）

```
時間  誰（process）    發生什麼
t0   shell            exec ~/.cargo/bin/cargo（rustup proxy）
t1   proxy            查目前的 toolchain → exec 真正的 cargo
t2   cargo            讀 Cargo.toml / Cargo.lock，算依賴圖：hello → util
t3   cargo            缺的 crate 從 crates.io 下載到 ~/.cargo/registry
t4   cargo            fork+exec rustc 編 util
t5   rustc(util)      原始碼 → AST → HIR → 型別檢查      ← 型別錯在這裡報
t6   rustc(util)      HIR → MIR → borrow check          ← 借用錯在這裡報
t7   rustc(util)      泛型展開 → LLVM IR → 最佳化 → .o（LLVM 在 rustc process 內）
t8   rustc(util)      包成 libutil.rlib（library 不 link）
t9   cargo            fork+exec rustc 編 hello，帶 --extern util=libutil.rlib
t10  rustc(hello)     前端：讀 util 的 metadata 做型別檢查、borrow check
t11  rustc(hello)     LLVM → hello.o；call util::foo 的位址是洞
t12  rustc(hello)     fork+exec cc，傳入 hello.o + libutil.rlib + std + 系統庫
t13  cc → ld          符號解析 → 排位址 → relocation 填洞 → 寫出執行檔
t14  cargo            fork+exec target/debug/hello

若是 cargo check：每個 crate 只做到前端並輸出 .rmeta，
t7–t8 與 t11–t14 不發生。
```

> **補強**（入庫時加，非這條線的導師內容）：t14 寫的是 cargo「fork+exec」執行檔，§13 t5 寫的是「exec」，兩處不一致。Unix 上 `cargo run` 應該是直接 exec 換映像、不留一個 cargo 當 parent；這次沒有在 Mac 上觀察，記在 checklist 待確認。

## 12. 建置模型：crate、feature、ABI、靜態與動態連結

§11 講的是三個 process 怎麼接力。這一節是照 §10 在 Mac 上實跑時問出來的名詞：被編譯的單位是什麼、`--features` 撥的是什麼、兩段機械碼靠什麼配合、函式庫的機械碼最後放在哪。標「實測」的是 2026-11-16 在這台 Mac 上量到的（專案名 `bevy-snake`、rustc 1.100.0、bevy 0.20.1；看的是 `target/` 的內容、`otool -L`、cargo 的 fingerprint 紀錄與直接執行的結果），其餘是依機制的說明。

### 12.1 crate 是編譯單位，不是產物

**核心規則**：crate 是 `rustc` 一次編譯的那一包原始碼：從一個根檔案出發，加上它用 `mod` 拉進來的所有檔案。自己的專案和第三方套件都是 crate。編出來的檔案是它的產物，不是它本身。

| 種類 | 根檔案 | 產物 |
|---|---|---|
| binary crate | `src/main.rs`（有 `main`） | 可執行檔 |
| library crate | `src/lib.rs` | 函式庫，形式由 `crate-type` 決定（§12.5） |

- 「binary」有兩個意思。binary crate 說的是原始碼的用途（會編成執行檔），a binary 說的是編出來的檔案。`bevy-snake` 是 binary crate，`target/debug/bevy-snake` 才是 binary。
- 編譯是「一個 crate 變一個產物」，連結是「多個產物接成一個執行檔」。§10 步驟 1 的兩個步驟在這裡各有了單位。
- package 和 crate 不是同一層：一個 package 是一份 `Cargo.toml`，裡面最多一個 library crate、可以有多個 binary crate。相依、feature、版本都掛在 package 上。

### 12.2 發佈的是原始碼，不是二進位

**核心規則**：crates.io 上放的是 `.rs` 原始碼，每台機器自己編。第一次建置久、之後快，都是從這一條來的。

| 生態 | 發佈的是什麼 | 誰把它變成機械碼 |
|---|---|---|
| Rust（crates.io） | 原始碼 | 使用者的機器，建置時 |
| Java（Maven） | bytecode（中繼碼） | JVM，執行時 |
| npm | JavaScript 原始碼 | JS 引擎，執行時 |
| C / C++（系統套件） | 多半是編好的函式庫加標頭檔 | 發佈者，事先 |

- 中繼形式（HIR、MIR、LLVM IR）只存在於 rustc 的記憶體裡，不是發佈形式（§11.3）。
- 編好的 `.rlib` 也不是完整的成品：泛型函式要等使用端決定具體型別才產生機械碼，所以 `.rlib` 的 metadata 裡留著泛型的定義，展開發生在用到它的那個 crate。Bevy 大量用泛型，自己的 crate 因此也不輕。
- 換到的：編譯器能跨 crate 最佳化和展開泛型、不必凍結 ABI（§12.4）、一份原始碼涵蓋所有平台與 feature 組合。付出的：每台機器都要自己編，相依的原始碼得拿得到。
- 實測：`Cargo.lock` 列了 573 個 package，這台 Mac 實際編了約 370 個；相依的 `.rlib` 在十幾分鐘內陸續完成，`target/` 長到 8.3 GB。

### 12.3 feature：編譯期的開關

**核心規則**：feature 是 crate 在自己的 `Cargo.toml` 宣告的具名開關，由使用它的人在建置時決定開不開。它在編譯期就定案，編完的產物裡沒有這個開關。開不開，那個 crate 都會被編；改變的是編成什麼樣子。

`--features bevy/dynamic_linking` 讀作「把相依 `bevy` 的 `dynamic_linking` 打開」。`/` 前面是相依的名字，不是路徑；自己 crate 的 feature 不帶 `/`。

| 開關能控制的 | 機制 |
|---|---|
| 一段程式碼編不編 | 原始碼標 `#[cfg(feature = "…")]`，沒開就在很早的階段整個拿掉 |
| 一個相依拉不拉進來 | 相依標成 optional，開了才下載與編譯 |

- `dynamic_linking` 屬於第二種。實測：cargo 的 fingerprint 紀錄裡，`bevy` 的相依是 `bevy_dylib` 與 `bevy_internal`；`bevy_dylib` 的相依只有 `bevy_internal`，產物是一個 `libbevy_dylib.dylib`。
- `#[cfg]` 管的是緊接在它下面的那一個完整結構（函式、struct、`mod`、`use`、欄位、陳述式），不是任意幾行。
- 沒開的程式碼不做型別檢查也不做 borrow check，壞了不會報。feature 多的專案要在 CI 把組合都編過，否則平常沒開的那一段會悄悄爛掉。
- `cfg!(feature = "…")` 是另一回事：它變成 `true` 或 `false`，`if` 裡面的程式碼照樣要能通過編譯。
- 寫在指令上還是寫進 `Cargo.toml`：寫進 `[dependencies]` 的 feature 每次建置都生效，包含 `--release`。只想在開發時開的放指令上，再用 alias 收起來（§10 步驟 13）。
- 同一套 `#[cfg]` 也用在別的條件上：`target_os`、`debug_assertions`、`test`。

### 12.4 ABI：機械碼之上的約定

**核心規則**：機械碼只保證 CPU 認得每一條指令，不保證兩段機械碼配合得起來。配合靠約定：參數放哪個暫存器、struct 的欄位在第幾個 byte、函式叫什麼名字。這套約定就是 ABI，可以有很多套（經驗 01 §3、§4）。

| 層 | 規定什麼 | Rust 和 C 一樣嗎 |
|---|---|---|
| ISA（例如 arm64） | 有哪些指令和暫存器 | 一樣，同一顆 CPU |
| ABI | 兩段機械碼怎麼配合 | 不一樣 |

同一個 `struct S { a: u8, b: u32, c: u8 }`：

| | 排列 | 大小 | `b` 在哪 |
|---|---|---|---|
| C ABI | 照宣告順序，`a` 後面補 3 bytes | 12 bytes | offset 4 |
| Rust ABI | 編譯器可以重排，把 `b` 放最前面 | 8 bytes | offset 0 |

呼叫方照一種排法讀、被呼叫方照另一種寫，兩段都是合法的 arm64 指令，合起來讀到的是垃圾。

| | C ABI | Rust ABI |
|---|---|---|
| 公開規格 | 有，平台文件寫死 | 沒有 |
| 跨編譯器版本 | 穩定 | 每一版都可以改 |
| 能表達的型別 | 整數、浮點、指標、`#[repr(C)]` 的 struct | 完整的 Rust 型別（泛型、trait、`String`、`Vec`、`Result`） |
| 符號名字 | 原樣 | 改寫成含 crate 名與雜湊的長名字 |

- 「C ABI」其實是平台的 ABI，只是用 C 的型別來描述。每個平台各一份（Apple arm64、Linux x86-64、Windows x86-64 都不同），所以編好的函式庫不能跨平台用，target triple 要把平台寫明（經驗 01 §5）。
- Rust 不凍結 ABI 是刻意的：保留重排欄位、改傳參方式的自由，換更小的 struct 和更多最佳化。代價是編好的 `.rlib` 與 `dylib` 只有同一版 `rustc` 能用，這也是 Rust 對 Rust 只發佈原始碼的原因（§12.2）。
- `extern "C"` 和 `#[repr(C)]` 的意思是「這個函式、這個 struct 改用平台公開的那套約定」。指令集沒變，變的是約定。
- 例外是 `std`：rustup 給的是預編好的，因為它和 `rustc` 綁在同一個 toolchain 一起發。`dynamic_linking` 的 dylib 能用也是同一個理由：同一台機器、同一次建置、同一個 `rustc`。

### 12.5 靜態與動態連結：機械碼最後放在哪

**核心規則**：靜態是建置時由 `ld` 把函式庫的機械碼抄進執行檔；動態是建置時 `ld` 只確認符號存在、記下「需要誰」，執行時由 `dyld` 載入。兩種在建置時都會經過 `ld`。§11.6 最後那張表是 Rust 的預設，這裡補上「library crate 可以編成哪幾種」和實測。

| `crate-type` | 產物 | 連結方式 | ABI | 給誰用 |
|---|---|---|---|---|
| `rlib`（預設） | `.rlib` | 靜態 | Rust | 別的 Rust crate |
| `dylib` | `.dylib` / `.so` | 動態 | Rust | 別的 Rust crate，同一版 `rustc` |
| `staticlib` | `.a` | 靜態 | C | 任何語言 |
| `cdylib` | `.dylib` / `.so` / `.dll` | 動態 | C | 任何語言 |

- crate 只有一種身分（原始碼）；`.rlib`、`.a`、`.dylib` 是同一份原始碼編出來的不同產物。`.dylib` 是裝著機械碼的檔案，它遵守某一套 ABI，本身不是 ABI。
- 想把編好的函式庫交給別人而不給原始碼，走的是下面兩列，介面只能用 C 表達得出來的東西。Rust 連到 macOS 系統函式庫是同一條路的反方向。

| | 靜態 | 動態 |
|---|---|---|
| 發佈要帶什麼 | 只有執行檔 | 執行檔加函式庫 |
| 函式庫改了 | 整個重新連結 | 換掉那個檔案 |
| 自己的程式改了 | 函式庫要重新抄一次 | `ld` 只處理自己那一塊 |

最後一列是開發時開 `dynamic_linking` 的理由：改的是自己的 crate，Bevy 沒變。

實測，開 `dynamic_linking` 編出來的 `bevy-snake`：

| 邊 | 連結方式 | 依據 |
|---|---|---|
| 自己的 crate ↔ Bevy | 動態 | 執行檔 382 KB，`libbevy_dylib.dylib` 230 MB；執行檔記的是 dylib 在 `target/` 底下的絕對路徑 |
| 自己的 crate ↔ Rust `std` | 動態 | 執行檔記的是 `@rpath/libstd-<hash>.dylib` |
| Bevy 內部的 crate 之間 | 靜態 | 幾百個 `.rlib` 被接進同一個 dylib |
| ↔ 系統函式庫 | 動態 | AppKit、Metal、QuartzCore、CoreAudio 等 12 個 framework，加上 libSystem、libobjc、libiconv；不論開不開都是 |

- 變成動態的有兩條邊，不只 Bevy 那一條。執行檔和 dylib 都用到 `std`，同一個 crate 在一個 process 裡只能有一份，所以 `std` 得搬到兩邊共用的 dylib 去（機制說明，沒有另外驗證）。
- §2 分層表列的四個 framework（AppKit、Metal、QuartzCore、CoreAudio）都在這份清單裡。
- 不經 `cargo run` 直接執行 `./target/debug/bevy-snake`，`dyld` 報 `Library not loaded: @rpath/libstd-<hash>.dylib`、`Reason: no LC_RPATH's found`，程式沒進到 `main`。找不到的是 `libstd`，不是 Bevy 的 dylib：後者記的是絕對路徑，前者記的是 `@rpath`，而執行檔裡一條 rpath 都沒有。`cargo run` 會替它設 `DYLD_FALLBACK_LIBRARY_PATH`（§13 t5）。
- 這就是 `dynamic_linking` 只能用在開發的具體樣子：執行檔離不開這台機器上的兩個位置。

### 12.6 建置快取、兩種設定檔、兩套 toolchain

**核心規則**：cargo 以「一個 crate 在一組設定下」為單位快取。設定包含 crate 版本、打開的 feature、profile、target 與 `rustc` 版本；任何一項不同就是另一個單位，另編一份放進另一個目錄，舊的不會被蓋掉。

| 改了什麼 | 重編範圍 |
|---|---|
| 自己的原始碼 | 只有自己的 crate，再重新連結 |
| 某個相依的 feature | 那個相依和所有用到它的 crate |
| profile 的 `opt-level` | 受那個 profile 管的 crate 和用到它們的 crate |
| `rustc` 版本、`cargo clean` | 全部 |
| dev 與 `--release` 之間切換 | 兩套各自獨立，第一次都要全編 |

- 實測：這套 toolchain 的配置是 `target/debug/build/<package>/<hash>/out/`，不是 §13 t2 寫的 `target/debug/deps/`；`<hash>` 是那組設定算出來的雜湊，旁邊的 `fingerprint/` 記著 feature、profile、相依與 `rustc`。292 個 package 只有一份 `.rlib`（相依從頭到尾只編過一次），44 個有兩到四份，分在不同的 `<hash>` 目錄並存。另有一批只含 `.rmeta` 的目錄，是 `cargo check` 留下的（§11.4）。
- `target/` 只增不減：切換過的每組設定都留著。清理靠 `cargo clean`。
- `opt-level` 是最佳化強度，0 到 3，數字越大編越久、跑越快。§10 步驟 7 把自己的 crate 設 1、相依設 3，依據是改動頻率：相依只編一次，付一次成本換每一幀的速度；自己的 crate 一直在改，要的是重編快。

| 檔案 | 描述什麼 | 例子 |
|---|---|---|
| `Cargo.toml`（manifest） | 這個 package 是什麼 | 名稱、相依、feature、`[profile.*]` |
| `.cargo/config.toml` | cargo 這個工具在這個目錄底下怎麼運作 | `[alias]`、預設 target、linker、環境變數 |

- 實測：`[alias]` 寫進 `Cargo.toml`，`cargo dev` 回 `error: no such command: dev`；搬到 `.cargo/config.toml` 就生效。cargo 從目前所在的目錄往上找這個檔案。這不是撞到保留字，也不是版本問題。
- 實測：同一台機器上有兩套 toolchain，rustup 的 1.100.0 和 Homebrew 的 1.87.0。專案是用前者編成功的；之後打 `cargo dev` 拿到的卻是後者，被 bevy 0.20.1 的 `rust-version` 擋下（`rustc 1.87.0 is not supported by the following packages`）。shell 照 `PATH` 的順序取第一個找到的；`rustc --version` 尾巴的 `(Homebrew)` 是辨認來源的線索（§10 步驟 2）。錯誤訊息建議的 `cargo update --precise` 是把相依降版去配合舊編譯器，這裡該做的是換回新的 toolchain。

### 12.7 可遷移的工程模式（續 §11.7）

| 模式 | 這一節的例子 |
|---|---|
| 編譯期決定與執行期決定 | feature 與 `#[cfg]` 在編譯期定案，產物裡沒有開關；設定檔與環境變數才是執行期的 |
| 穩定的公開介面與可變的內部介面 | C ABI 公開且凍結，誰都能接；Rust ABI 不對外承諾，換來改實作的自由 |
| 發佈原始碼與發佈二進位 | crates.io 發原始碼：能跨 crate 最佳化、不必凍結 ABI，代價是每台機器自己編 |
| 以設定的雜湊為鍵的快取 | cargo 把 crate、feature、profile、target、`rustc` 版本算成雜湊；設定不同就並存，不覆蓋 |
| 依改動頻率分開最佳化 | 相依 `opt-level = 3` 只付一次；自己的 crate `opt-level = 1` 換重編速度 |
| 成本放在建置期還是啟動期 | 靜態連結把函式庫抄進執行檔，發佈自足；動態連結把接合留到啟動，開發時重編快 |
| 「描述產品」與「設定工具」分開 | `Cargo.toml` 與 `.cargo/config.toml` |
| 同名工具並存時先找到的贏 | `PATH` 上 Homebrew 的 `rustc` 排在 rustup 前面 |

## 13. 思維鏈：`cargo run --features bevy/dynamic_linking` 到畫出第一幀

```
t    誰在跑（process / thread）        mode           跑誰的程式碼                  發生什麼
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
t0   zsh                               user → kernel  zsh                           fork + exec ~/.cargo/bin/cargo；那是 rustup 的 proxy，
                                                                                    再 exec toolchain 裡真正的 cargo
t1   cargo                             user           cargo                         讀 Cargo.toml 與 Cargo.lock，算出相依圖；缺的 crate
                                                                                    從 crates.io 下載到 ~/.cargo/registry
t2   cargo → 多個 rustc 子 process      user           rustc                         由葉到根，一個 crate 一個 rustc process，平行度 = 核心數；
                                                                                    產物 .rlib 放進 target/debug/deps
t3   rustc（bevy_dylib）→ cc → ld       user           Apple ld                      把整個 Bevy 連成一個 libbevy_dylib.dylib。只在第一次做
t4   rustc（snake）→ cc → ld            user           Apple ld                      你的 crate 編完；ld 只記下「需要 libbevy_dylib、libstd、
                                                                                    系統 framework」，產出 Mach-O 執行檔 target/debug/snake
t5   cargo                             user → kernel  cargo                         設 DYLD_FALLBACK_LIBRARY_PATH，exec target/debug/snake
t6   snake 的 main thread              kernel         XNU（execve）                  換映像：把 Mach-O 映射進位址空間，控制權交給 dyld
t7   main thread                       user           dyld                          載入 libbevy_dylib、libstd、AppKit、Metal、CoreAudio…，
                                                                                    綁定符號，跳到 main
t8   main thread                       user           你的 main → bevy_app           App::new()；add_plugins 當場執行每個 plugin 的 build()：
                                                                                    登記 system 與 resource，TaskPoolPlugin 在這裡開好
                                                                                    worker thread。還沒有任何 system 在跑
t9   main thread                       user           bevy_winit → winit → AppKit    .run() 把控制權交給 winit 的 event loop，結束前不 return；
                                                                                    建立視窗（AppKit 透過 Mach IPC 請 WindowServer 這個
                                                                                    process 配一塊畫面）
t10  main thread                       user (+kernel) bevy_render → wgpu → Metal     視窗有了才完成 renderer 初始化：選 Metal backend、拿到
                                                                                    GPU 裝置，印出 AdapterInfo；另開一條 "Render thread"
t11  main thread + compute worker      user           bevy_ecs executor → 你的 setup  第一次 app.update()：跑 Startup。setup 裡的
                                                                                    commands.spawn 只是排隊，到同步點才寫進 World
t12  main thread                       user           winit ← AppKit                每一圈先收事件（鍵盤、視窗大小），轉成 Bevy 的輸入資料
t13  main thread + compute workers     user           bevy_ecs executor → 各 system   跑主 schedule：First → PreUpdate → StateTransition →
                                                                                    RunFixedMainLoop（FixedUpdate 零到多次）→ Update →
                                                                                    SpawnScene → PostUpdate → Last；讀寫不衝突的 system
                                                                                    同時跑在不同 worker 上
t14  main thread                       user           bevy_render（extract）         把這一幀畫面需要的資料從 main world 抄一份到 render world，
                                                                                    再把 render world 交給 Render thread
t15  Render thread                     user           bevy_render → wgpu → Metal     排序、組 draw 命令、編成 Metal command buffer 送出。
                                                                                    同一時間 main thread 已經回到 t12 跑下一幀
t16  —（GPU 硬體）                      —              GPU                           執行命令，畫進視窗的 drawable（note 03 那條鏈的下半段，
                                                                                    這裡不展開）
t17  WindowServer（另一個 process）     user           WindowServer                  下一次螢幕更新時把這個視窗的畫面合成到螢幕上
```

- t0–t5 是工具鏈，t6 之後才是 runtime。建置期出現的 process（cargo、rustc、ld）在遊戲跑起來時一個都不在。
- t8 的 `add_plugins` 和 t9 的 `.run()` 是兩件事：前者登記，後者才開始轉。這就是「main loop 屬於引擎」的具體樣子。
- t13 沒有一個「中央管理者」在憑空派工。派工的是 executor 這段程式碼，它由 main thread 在 `app.update()` 裡啟動，依每個 system 宣告的讀寫決定誰能同時跑。
- t14 是模擬和渲染之間唯一的交接點。之後 main thread 算第 N+1 幀、Render thread 畫第 N 幀，兩邊同時進行，所以畫面比模擬晚一幀。
- 整條時間軸只有 t6 明確在 kernel mode。t9、t10、t15 都會為了 IPC 和送 GPU 命令短暫進 kernel，細節是 note 03 與 note 05 §13 的內容。

## 14. 解法對照

「想獨立做一款遊戲，引擎挑哪個」的幾條路：

| 選擇 | 什麼情況選它 | 代價 |
|---|---|---|
| A. Bevy，第一款做貪吃蛇（本篇採用） | 想用 Rust；系統驅動、不靠手擺關卡的類型；PC 與網頁；目的是先確認自己喜不喜歡做遊戲這件事 | 沒有編輯器、UI 年輕、每隔幾個月照遷移指南改一輪、agent 常寫出舊版 API；手機與主機的輸出不如 Godot、Unity 成熟 |
| B. Godot | 以商業上架為目標；內容驅動、吃編輯器的類型；PC 與網頁 | GDScript 訓練資料少，agent 偶爾寫出舊版 3.x 語法；3D 不如 Unreal；主機要靠第三方移植 |
| C. godot-rust | 不想放棄 Rust，又要編輯器和 UI：遊戲邏輯用 Rust 寫，編輯器和 UI 交給 Godot | 這條線只提到有這條路，沒有往下談 |
| D. Unity | 主攻手機，或一開始就要上主機 | 很多設定要人在編輯器裡點；年營收或資金超過 20 萬美元要買 Pro |
| E. Cocos Creator | 目標是亞洲手遊與小遊戲平台 | 目標是那個市場才選它 |

- 判斷 A 走不走得下去的方法是一次有期限的實驗：兩週做完一個極小的遊戲。兩週後大部分時間花在做遊戲就繼續，大部分時間在跟 UI 和工具搏鬥就換 B。貪吃蛇基本版之後刻意加分數顯示、結束畫面、選單、音效，就是為了讓這個判斷有依據（§9）。
- 選 A 不是因為它最穩：以商業上架為目標，Godot 比較穩；Bevy 適合拿來確認自己喜不喜歡做遊戲這件事。
- 不管選哪一條，引擎都很少是成敗的瓶頸，選一個成熟的就該往下走；會改變答案的是 §5 那三個變數。
- 選了 A 之後改版的痛怎麼擋（鎖版、規則檔、編譯器把關各擋得住什麼），對照表在 §8。

## 15. 陷阱清單：錯在哪、正解是什麼、為什麼

**引擎的模型**

1. ❌ Godot 是「PC 端」的引擎 → ✅ 編輯器跑在桌機三平台，輸出目標另外算（桌機三平台加 Android、iOS、網頁） → 引擎是 runtime 加工具鏈，兩半的平台各自獨立；問「它是哪個端的」要先講清楚問的是哪一半。
2. ❌ 引擎選什麼都大差不差 → ✅ 對遊戲成敗影響很小，但目標平台、遊戲類型、主力是不是 agent 這三個變數會改變答案 → 主攻手機或主機卻選了 Bevy、想讓 agent 當主力卻選了靠 Blueprint 的 Unreal，都是開工之後才會撞到的牆。
3. ❌ 現在都是 code agent 在寫，所以沒什麼難度 → ✅ agent 寫掉的是程式，而程式本來就不是最難的；好不好玩、風格一致、讓人知道這款遊戲存在，這三件它沒碰到 → 大家都有的能力不會是優勢，差異只落在 agent 做不到的那幾項。
4. ❌ 引擎要花很多錢 → ✅ 全部可以免費開始，差別在賺到錢之後；真正的支出是美術音樂外包和平台抽成 → 開源的四個永遠不收費，Unity、Unreal 的門檻是年營收 20 萬與累計 100 萬美元。

**Bevy 與 ECS**

5. ❌ 「UI 不強」= 做不出 UI → ✅ 是年輕且只能寫程式：沒有視覺化編輯器、基本元件 0.20 才有、動畫要自己補 → 做得到，但調一個間距要改程式重編，而調 UI 正需要快速來回。
6. ❌ 「六個平台都支援」= 六個平台一樣好出 → ✅ 桌機與網頁可以放心；手機沒有一鍵輸出，商店 SDK 要自己橋接；主機沒有官方支援 → 「支援」只表示引擎能在上面跑，不含打包、上架、內購那一段。
7. ❌ 用規則檔綁住 agent，版本迭代的問題就解決了 → ✅ 解決大部分；剩兩件擋不住：新功能 agent 沒學過只能靠文件，編譯器只驗能不能編、不驗好不好玩 → 規則檔約束的是 agent「怎麼查」，補不了它「沒看過」的東西。
8. ❌ system 自己會跑，或有個中央管理者在派工 → ✅ system 是普通函式；每幀是 main thread 啟動 executor，executor 依每個 system 宣告的讀寫，把不衝突的丟到 worker thread → 主詞講不清楚，就推不出「哪兩個 system 會同時跑、哪兩個一定有先後」。
9. ❌ query 回來的順序就是生成的順序 → ✅ ECS 不保證順序；需要順序（蛇身）就自己用 resource 存一個 `Vec<Entity>` → component 是依型別連續存放的，不是依建立時間。
10. ❌ `commands.spawn` 之後下一行就查得到那個 entity → ✅ `Commands` 是延後執行，到同步點才寫進 World → system 可能正和別的 system 平行在跑，當場改結構會破壞別人的 query。
11. ❌ 讓 agent 一次生成整個貪吃蛇 → ✅ 一個 system 一個 system 自己寫，agent 負責解釋和除錯 → 它幾秒就能吐出完整程式碼，但你什麼都學不到，而且很可能是舊版寫法。

**版本與建置**

12. ❌ 在 `Cargo.toml` 寫 `bevy = "0.20"` 就是鎖定版本 → ✅ 那是可接受的範圍；鎖住版本的是 `Cargo.lock`，要 commit，並禁止 `cargo update` → 少了 lock，重新解析時可能拿到不同的 patch 版。
13. ❌ `cargo add bevy@0.20` 之後 clone `v0.20.0` 當參考 → ✅ 對齊 `Cargo.lock` 裡實際的版本（現在是 0.20.1，tag `v0.20.1`） → 參考的原始碼和實際編的不是同一版，就會抄到對不上的寫法。
14. ❌ rustup 裝好就可以了 → ✅ 先 `rustup update stable` → Bevy 的 MSRV 是最新 stable，實測 0.20.1 連 rustc 1.97.0 都拒絕，要 1.97.1。
15. ❌ macOS 也要換 lld 或 mold、開 Cranelift 才編得快 → ✅ 只開 `dynamic_linking` 和 dev profile 就好 → macOS 預設 linker 已經夠快，Cranelift 在 macOS 上可能讓 Bevy crash。
16. ❌ `dynamic_linking` 一直開著，連發佈也用 → ✅ 只在開發時開 → 開了之後執行檔離不開那個 dylib 和 toolchain 裡的 libstd，搬到別台機器跑不起來。
17. ❌ 視窗跳不出來，先更新顯卡驅動 → ✅ 那是 Windows、Linux 的建議；macOS 的 Metal driver 跟著 OS 走，沒有可以單獨更新的東西 → 該看的是 terminal 的錯誤訊息。

**這條線順帶校正的（引擎之外）**

18. ❌ 「端遊」指手機上的遊戲（用詞） → ✅ 端遊是客戶端遊戲，也就是 PC；手機上的是手遊。
19. ❌ 原型（prototype）是上架的小產品，所以大家做原型會塞爆市場、會被抄 → ✅ 原型是只有自己和幾個測試者玩過的內部試作，不進市場 → 目的是在投入一年之前先確認核心好不好玩。
20. ❌ 人手一機，所以手機是最好的潛在市場 → ✅ 使用者最多不等於機會最大：手機商店的曝光幾乎全靠買量、免費加內購需要長期營運、仿製速度最快 → 比較穩的順序是先在 PC 驗證，再移植手機。

**續：為什麼要裝 Command Line Tools（§10 步驟 1）**

21. ❌ 要裝 Xcode（用詞） → ✅ 要裝的是 Xcode Command Line Tools：只有命令列工具與 SDK 的小套件，不含 IDE。
22. ❌ Rust compile 完還是要跟 C 做 dynamic linker → ✅ 兩件事：建置時 `ld` 把 Rust crate 靜態接成執行檔，對系統函式庫只記名字；執行時 `dyld` 才把 libSystem 接上。沒有 C 原始碼被編譯，系統函式庫對外講的是 C ABI → 混成一個就推不出「為什麼做好的遊戲拿到沒裝 Command Line Tools 的 Mac 照樣能跑」：`ld` 只在建置時出場，`dyld` 是系統內建。
23. ❌ `cc` 在做 ABI 串接 → ✅ `cc` 只是用對的參數啟動 `ld` 的手段；ABI 是約定，`rustc` 產生機械碼時就照它擺好了，`ld` 只照符號名字接 → `ld` 不檢查擺法，所以 ABI 沒對齊時連結照過、執行才壞（經驗 01 §3）。
24. ❌ 裝這包是為了取得 `rustc` 跟 clang；不裝就是自己配路徑 → ✅ `rustc` 來自 rustup，這包給的是 `cc`、`ld`、macOS SDK、`git`；不裝是東西不存在，不是找不到 → 路徑只能指向已經在機器上的檔案；linker 有替代品，SDK 只有 Apple 發。

**續：toolchain 與編譯流程（§11）**

25. ❌ Cargo 會把 crate 連接在一起 → ✅ Cargo 只做安排（順序、路徑、參數）；連接是 linker 做的，而 linker 是 rustc 叫起來的 → 為什麼：三個 process 各做各的，`cargo build -v` 看得到 Cargo 只是在下 rustc 指令。
26. ❌ 不用 Cargo 就要「標註相依範圍」再編譯 → ✅ 要自己按順序編好每個依賴，再用 `--extern` 把產物路徑告訴 rustc → 為什麼：rustc 一次只編一個 crate，不知道其他 crate 在哪。
27. ❌ `cargo check` 只檢查型別 → ✅ 它做完整個前端，包含 borrow check → 為什麼：借用錯誤在 check 就會報，不必等 build。
28. ❌ check 省掉的是「組語」那段 → ✅ 省掉的是 codegen 與 link → 為什麼：rustc 通常不經過文字組語，LLVM 直接寫出 `.o`。
29. ❌ LLVM 是一支獨立的 C++ 程式，裝在 Xcode Command Line Tools 裡 → ✅ LLVM 是函式庫，隨 rustup 的 toolchain 來，住在 rustc process 裡；Xcode CLT 只借出 linker → 為什麼：rustc 在記憶體裡直接呼叫 LLVM 的 API，clang 裡那份 LLVM 是另一份。
30. ❌ Rust 提供了 LLVM IR → ✅ LLVM IR 是 LLVM 定義的格式，Rust 團隊寫的是「MIR → LLVM IR」的翻譯器 → 為什麼：共同格式由後端定義，各語言前端負責翻過去。
31. ❌ LLVM 就是把程式轉成目的檔 → ✅ 它的輸入是 LLVM IR 而不是 Rust，而且先最佳化才產生 `.o` → 為什麼：LLVM 完全不認識 Rust，`Result`、借用、泛型到這層都已消失。
32. ❌ LLVM 全名有 Virtual Machine，所以是執行期的虛擬機 → ✅ 那是歷史名稱，現在只是專案名；它不像 JVM 那樣在執行期跑你的程式 → 為什麼：唯一留下的意思是 LLVM IR 像一台假想機器的組語。
33. ❌ HIR、MIR、LLVM IR 是三種語言或三種檔案 → ✅ 是 rustc 記憶體裡同一支程式的三個改寫版本，High / Mid 指離原始碼多近 → 為什麼：每一層是為了讓某一種檢查或轉換好做而存在。

**續：實跑之後的建置模型（§12）**

34. ❌ crate 就是第三方套件；或 crate 是編出來的 binary、compile 的 output 都是 crate → ✅ crate 是 `rustc` 一次編譯的那包原始碼，自己的專案也是一個；執行檔、`.rlib`、`.dylib` 是它的產物 → 分不開就讀不懂「library crate 編成 `rlib` 還是 `dylib`」這種句子：同一個 crate 可以有好幾種產物。
35. ❌ `--features bevy/dynamic_linking` 是「針對這個 crate 做 compile」，開了才需要 compile → ✅ `bevy` 不論如何都會被編；feature 是 `bevy` 自己宣告的編譯期開關，改變的是編成什麼樣子（這裡是多拉一個相依，把 Bevy 包成 dylib） → 開關在相依那邊、不在自己的 crate 裡，`/` 前面是相依的名字。
36. ❌ library crate 可能是一包中繼碼 → ✅ 發佈的是 `.rs` 原始碼；中繼形式只在 rustc 的記憶體裡，`.rlib` 是機械碼加 metadata → 這是第一次建置要十幾分鐘的原因，也是泛型能跨 crate 展開的前提。
37. ❌ 都編成機械碼了，ABI 就只有一種，不該再分 Rust 和 C → ✅ 指令集是同一套，約定是兩套；C ABI 是平台公開且凍結的那一套，Rust ABI 沒有對外承諾 → 兩段合法的機械碼照不同的排法讀寫同一個 struct，讀到的是垃圾。
38. ❌ 動態連結 = 目的檔透過 linker 封裝成執行檔 → ✅ 那是靜態；動態是 `ld` 只記下需要誰，函式庫留在另一個檔案，執行時由 `dyld` 載入 → 兩種都經過 `ld`，差在機械碼最後放在哪。
39. ❌ `.dylib` 是「runtime 被載入的 ABI」 → ✅ `.dylib` 是裝著機械碼的檔案，ABI 是它對外遵守的約定 → 同樣是 `.dylib`，`dylib` 講 Rust ABI、`cdylib` 講 C ABI，能給誰用完全不同。
40. ❌ `[alias]` 寫在 `Cargo.toml`；`cargo dev` 報 `no such command` 是撞到關鍵字或版本問題 → ✅ alias 屬於 `.cargo/config.toml` → `Cargo.toml` 描述 package，`.cargo/config.toml` 設定 cargo 這個工具，cargo 不會從前者讀 alias。
41. ❌ 帶 `dynamic_linking` 的執行檔直接跑，`dyld` 找不到的是 `libbevy_dylib` → ✅ 找不到的是 `libstd-<hash>.dylib`（實測） → Bevy 的 dylib 記的是絕對路徑，`std` 記的是 `@rpath` 而執行檔沒有 rpath；開了這個 feature，`std` 也跟著變成動態。
42. ❌ 改了 feature 或 profile，整個 `target/` 重來、舊的被蓋掉 → ✅ 只重編受影響的單位，新舊以不同的雜湊並存 → 所以切回原來的設定不用重編，也所以 `target/` 只增不減。
43. ❌ `rustup update` 過了，打 `cargo` 用到的就是新版 → ✅ shell 照 `PATH` 取第一個；Homebrew 那份排在前面時用到的是舊版（實測：1.87.0 對 1.100.0） → 看 `rustc --version` 尾巴有沒有 `(Homebrew)`，以及 `which -a rustc` 的順序。
