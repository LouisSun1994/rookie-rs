# rookie-rs

Rust 學習日誌。從零開始，記錄概念、踩坑與修正過的誤解。

## 背景

- 熟悉 Go（GMP 排程模型）、後端 / 基礎設施、遊戲系統
- 學習方式：概念優先（conceptual-first），先建立全貌再動手實作

## 目錄

| 路徑 | 內容 |
|---|---|
| `notes/` | 學習筆記，依序編號 `NN-主題.md` |

### 筆記索引

| 編號 | 主題 | 日期 |
|---|---|---|
| [01](notes/01-memory-model-async-runtime-os.md) | 記憶體模型、ownership、smart pointer、async runtime、作業系統層機制 | 2026-10-07 |
| [02](notes/02-tokio-runtime-os-scheduling.md) | Tokio 多執行緒 runtime、Task 與 Waker、epoll、OS 兩層排程、中斷 | 2026-10-08 |

## 階段路線圖

| 階段 | 內容 |
|---|---|
| 🥚 小白 | 環境、cargo、變數、型別、控制流程、函式、struct、enum、match |
| 🐣 入門 | ownership、borrowing、slice、String vs &str、Vec/HashMap、Option/Result、錯誤處理、module |
| 🦀 進階 | trait 與 generics、lifetime、closure、iterator、Box/Rc/RefCell/Arc、測試 |
| 🦞 熟練 | thread/channel/Mutex、async/tokio、trait object、效能分析、serde/clap/anyhow/axum |
| 🐊 巨鱷 | unsafe/FFI、macro、記憶體佈局、零成本抽象原理、API 設計、讀 std 原始碼 |

目前位置：🥚 小白（尚未動手寫程式，已概念性探索到 🦀/🦞 的 smart pointer、async、concurrency）。

## 慣例

- 筆記用繁體中文，程式碼與 commit message 用英文
- 每篇筆記開頭標記日期與當時的階段定位
- 被校正過的誤解集中記在各篇的「誤解修正記錄」節，不刪舊的錯誤理解

## 授權

個人學習筆記，保留所有權利。歡迎閱讀，請勿轉載或改作。
