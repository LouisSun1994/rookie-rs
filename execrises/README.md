# exercises

Rust 動手練習。一題一個 binary：

- 題目、實驗紀錄、問答與複盤：`questions/qNN_<slug>.md`
- 程式碼：`src/bin/qNN_<slug>.rs`

## 進度

🐣 入門

- [x] [q01：Collatz 步數](questions/q01_collatz.md) — 變數、`mut`、整數型別、函式、`loop`/`while`、expression
- [x] [q02：字串的三種簽名](questions/q02_strings.md) — `String` vs `&str`、move、`&` / `&mut`
- [x] [q03：玩家背包](questions/q03_inventory.md) — `struct`、`impl`、`&self` / `&mut self` / `self`、`Vec`、`Drop`
- [x] [q04：指令解析器](questions/q04_commands.md) — `enum`、`match`、`Option`
- [x] [q05：RPN 計算機](questions/q05_rpn.md) — `Result`、`?`、自訂 error、`checked_*`

🦀 進階

- [ ] [q06：字頻統計](questions/q06_wordfreq.md) — `HashMap`、iterator、closure
- [ ] [q07：mini grep](questions/q07_minigrep.md) — module 拆檔、檔案 I/O、CLI 參數、結束代碼、單元測試
- [ ] [q08：圖形與 trait](questions/q08_shapes.md) — trait、generics、`impl Trait` vs `dyn Trait`
- [ ] [q09：Tokenizer](questions/q09_tokenizer.md) — 持有 `&str` 的 struct、lifetime、自己實作 `Iterator`
- [ ] [q10：鏈結串列與樹](questions/q10_pointers.md) — `Box`、`Rc`、`Weak`、`RefCell`

🦞 熟練

- [ ] [q11：平行字頻統計](questions/q11_parallel.md) — thread、channel、`Arc<Mutex<T>>`、scoped thread
- [ ] [q12：echo 與聊天室 server](questions/q12_chat.md) — tokio、`select!`、broadcast，接回 notes/02
