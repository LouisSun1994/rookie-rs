# Experience 03 — 遊戲引擎、Bevy 與 ECS：想獨立做一款遊戲，引擎到底是什麼、該挑哪個

> 經驗不是筆記：起點是一個實際撞到的問題，往下挖到能解釋它為止。這一篇撞到的不是錯誤訊息，是一個卡住的決定：想獨立做一款遊戲，卻說不清「引擎」到底是什麼、該挑哪個。從引擎一覽與費用起步，經過「code agent 時代引擎還有沒有差」，落到 Bevy（Rust 寫的 ECS 引擎）的現況、ECS 的模型、版本怎麼鎖，最後定下第一款做貪吃蛇，並把 macOS 上從零到第一個視窗的建置步驟排出來。
> 核心收穫是四句話：**引擎 = 執行期的一包函式庫（runtime）+ 開發期的工具鏈**，兩半各有各的平台；**引擎很少是成敗的瓶頸，但目標平台、遊戲類型、誰是寫程式的主力這三個變數會改變答案**；**ECS 是一個每幀被查詢和更新的記憶體內資料庫**，system 只是函式，跑它的是 executor；**鎖住版本的是 `Cargo.lock`，不是 `Cargo.toml`**。
> 記錄日期：2026-11-13｜當時定位：🥚 小白，第一次準備動手寫程式。
> 起因：想獨立做一款遊戲，第一步選引擎就卡住：有哪些、要不要錢、「Godot 是 PC 端的嗎」、code agent 都能寫程式了引擎還有沒有差。
> 接在哪裡：[note 03](../notes/03-gpu-stack-driver-kernel.md)（「引擎」這個詞、GPU 那條鏈的下半段）、[note 04 §4](../notes/04-borrow-exclusivity-lifetime-elf.md)（多讀 XOR 單寫）、[經驗 01 §5、§7](01-libc-abi-alpine.md)（target triple、動態載入器）、[note 05 §13](../notes/05-macos-sandbox-seatbelt-proc-cred.md)（Mach）。OS 藍圖：子系統 ①（一個 process 裡的 main thread、worker、Render thread）、②（dyld 載入 dylib 與 framework）。
> 時效：Bevy 0.20 於 2026-10-08 釋出，`cargo add bevy@0.20` 解析到 0.20.1；Godot 穩定版 4.7（2026-06）；各引擎的授權費用。皆為 2026-11 時的狀態，費用以官網為準。
> 來源與驗證狀態：§3–§9 來自這條線的對話。§2 分層與 §11 思維鏈是整理時由導師補上的骨架，對話裡沒有逐條走過；其中執行緒與 schedule 的順序對照過 Bevy 0.20.1 原始碼。§10 的兩段程式碼與 `Cargo.toml` 在 Linux 上對 bevy 0.20.1 跑過 `cargo check`（rustc 1.97.0 加 `--ignore-rust-version`）；**§10 的指令還沒在 Mac 上實跑過**，標「推論」的是依機制推出來、沒有重現過的。§1 事件與 §12 解法對照是整理進 repo 時依本文其他章節重排的，沒有新增內容。§10 步驟 1 的「為什麼要裝它」、§1 的 t7 與陷阱 21–24 是入庫當天的追問，在整理 repo 的 session 裡問答，不是原本那條線的導師，也沒有在 Mac 上逐項驗證。沒有腦筋急轉彎留在本文；待實跑確認的推論與追問留下的兩題記在 [`blueprints/os-checklist.md`](../blueprints/os-checklist.md)。

---

## 0. 一句話版本

遊戲引擎是兩樣東西：連結進你執行檔、擁有 main loop 並每幀呼叫你程式碼的 runtime，以及只在開發期出現的工具鏈（編輯器、匯出器）。挑引擎時它很少決定遊戲成敗，真正會改變答案的是目標平台、遊戲是系統驅動還是內容驅動、以及主力是不是 code agent。Bevy 目前只有 runtime 那一半：純 Rust、核心是 ECS，沒有編輯器、UI 年輕、每幾個月一次破壞性改版；用 `Cargo.lock` 鎖版加上 agent 規則檔可以擋掉大部分改版的痛。macOS 上的系統相依只有 Xcode Command Line Tools，因為 `rustc` 不自己做連結，而視窗、繪圖、音訊的 framework 都在系統 SDK 裡。

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
```

當時的模型，每一條都會讓選擇走偏（括號是 §13 陷阱清單的編號）：

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

陷阱 8–17 屬於 ECS 與建置那一側，跟著 §7–§10 收在陷阱清單。

往下挖的順序：先把整條鏈的層次畫出來（§2）→ 引擎是哪兩樣東西（§3）→ 有哪些、各要多少錢（§4）→ 怎麼選（§5）→ Bevy 現在的樣子（§6）→ ECS 的模型（§7）→ 版本怎麼鎖（§8）→ 第一款做什麼（§9）→ macOS 上怎麼從零建起來（§10）→ 一條指令到第一幀的完整時間軸（§11）→ 回頭把幾條路並排（§12）。

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

每幀的 schedule 順序（0.20.1 原始碼）：`First → PreUpdate → StateTransition → RunFixedMainLoop → Update → SpawnScene → PostUpdate → Last`。`RunFixedMainLoop` 會依經過的時間把 `FixedUpdate` 跑零到多次。渲染不在這條主 schedule 裡，在另一個 sub-app、另一條 thread 上（§11）。

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

## 11. 思維鏈：`cargo run --features bevy/dynamic_linking` 到畫出第一幀

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

## 12. 解法對照

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

## 13. 陷阱清單：錯在哪、正解是什麼、為什麼

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
