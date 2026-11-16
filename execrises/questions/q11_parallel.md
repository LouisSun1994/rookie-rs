# q11：平行字頻統計

檔案：[`src/bin/q11_parallel.rs`](../src/bin/q11_parallel.rs)

## 背景

同一份資料用四條 thread 一起數，寫三種「把結果合起來」的方式，量時間，並確認答案和單執行緒版本完全一樣。

這題一律用 `--release` 跑。

## 資料

用這個函式產生固定的測試資料（每次執行結果都相同）：

```rust
fn make_lines(n: usize) -> Vec<String> {
    const WORDS: [&str; 8] = ["crab", "borrow", "move", "trait", "heap", "stack", "thread", "await"];
    let mut seed: u64 = 42;
    let mut lines = Vec::with_capacity(n);
    for _ in 0..n {
        let mut line = String::new();
        for _ in 0..10 {
            // 刻意讓它溢位繞回，所以用 wrapping_*
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            line.push_str(WORDS[(seed >> 33) as usize % WORDS.len()]);
            line.push(' ');
        }
        lines.push(line);
    }
    lines
}
```

`make_lines(200_000)` 共兩百萬個字，正確答案：

| 字 | 次數 |
|---|---|
| await | 249868 |
| borrow | 249786 |
| crab | 249584 |
| heap | 250420 |
| move | 250431 |
| stack | 249637 |
| thread | 250178 |
| trait | 250096 |

## 要做的事

1. `fn count(lines: &[String]) -> HashMap<String, usize>`：單執行緒版本，當作基準。
2. 版本 A，共用一個 map：四條 thread 共用一個 `Arc<Mutex<HashMap<String, usize>>>`，每數到一個字就 lock 一次、加一。
3. 版本 B，channel：每條 thread 先數出自己的 `HashMap`，透過 `std::sync::mpsc::channel` 送回來，由 main thread 合併。
4. 版本 C，scoped thread：用 `std::thread::scope`，每條 thread 直接借用 `lines` 的一段，不用 `Arc`。
5. 每條 thread 開始工作時印出 `std::thread::current().id()`。
6. `main` 對四個版本各自用 `std::time::Instant` 量時間並印出，用 `assert_eq!` 確認 A、B、C 的結果都等於基準。

## 限制

- 不准出現 `unsafe`
- 每個 `spawn` 出來的 thread 都要被 `join`（版本 C 由 `scope` 負責）

## 實驗

每個都先預測，再試。

- A. 把版本 A 的 `Arc` 全部換成 `Rc`
- B. 把版本 A `thread::spawn(move || …)` 的 `move` 拿掉
- C. 版本 B：main thread 手上原本的那個 `tx` 不要 `drop`，直接開始從 `rx` 收。
  程式卡住時，開另一個終端機看這個 process 的 CPU 使用率
- D. 把版本 A 改成「每條 thread 先數完自己的 map，最後只 lock 一次合併進去」，和原本每個字 lock 一次的版本比時間

## 在註解裡回答（先寫預測，再跑）

1. 實驗 A：「`Rc` 不能跨 thread」這條規則是誰檢查的、在什麼時期檢查的？規則本身寫在哪裡？kernel 知不知道這件事？
2. 實驗 B：編譯器擔心的是什麼情況？`move` 改變了 closure 裡存的東西嗎？
3. 實驗 C：卡住的時候 main thread 處於什麼狀態，是排在 run queue 裡等 CPU，還是睡在某個 wait queue 上？它在等誰叫醒它？為什麼永遠等不到？
4. 四個版本的時間由快到慢是什麼順序？版本 A 如果比單執行緒還慢，時間花在哪裡？實驗 D 的結果支持你的解釋嗎？
5. `lock()` 回傳一個 guard。解鎖的動作是誰在什麼時候做的？如果一條 thread 在持有 guard 時 panic，其他 thread 之後 `lock()` 會拿到什麼？
6. 版本 C 為什麼不需要 `Arc`、也不需要 `move` 整份資料？`scope` 向編譯器保證了什麼？

## 交卷

commit 之後說「複盤 q11」。
