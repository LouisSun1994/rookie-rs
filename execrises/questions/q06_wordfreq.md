# q06：字頻統計

檔案：[`src/bin/q06_wordfreq.rs`](../src/bin/q06_wordfreq.rs)

## 背景

前面幾題限制你只能用 `for`。這題把限制解開：同一件事用 `for` 寫一次、用 iterator 鏈寫一次，並排比較。
iterator 鏈裡傳進去的 `|x| …` 是 closure（閉包，可以抓住外面變數的匿名函式）。

## 資料

把這段文字直接放進程式：

```rust
const TEXT: &str = "\
The crab walks sideways because the crab has no reason to walk straight.
The borrow checker is not your enemy; the borrow checker is your first reviewer.
A value has one owner, and when the owner goes away the value goes away.
You can lend a value to many readers or to one writer, but not both.
The compiler is strict so that the program at runtime does not have to be.";
```

## 要做的事

1. `fn tokenize(text: &str) -> Vec<String>`：以「不是英文字母的字元」切開，去掉空字串，全部轉小寫。應該得到 72 個字。
2. `fn count_loop(words: &[String]) -> HashMap<String, usize>`：用 `for` 和 `entry` API 計數。
3. `fn count_iter(words: &[String]) -> HashMap<String, usize>`：函式內不准出現 `for`，用 iterator 方法完成（例如 `fold`）。
4. `fn top_n(map: &HashMap<String, usize>, n: usize) -> Vec<(&str, usize)>`：依次數由大到小排序，次數相同依字母順序，取前 n 個。
5. 找出最長的字（長度相同取字母順序最前面的），寫兩個版本：
   - `longest_loop`：`for` 加自己維護的「目前最佳」變數
   - `longest_iter`：iterator 方法
6. `main` 裡宣告 `let min_len = 4;`，用一個抓住 `min_len` 的 closure 配 `.filter()`，算出 72 個字裡長度大於等於 `min_len` 的有幾個。
7. `main` 確認兩個計數版本結果相等（`assert_eq!`），並印出：

   | 項目 | 預期 |
   |---|---|
   | 不重複的字數 | `46` |
   | 前 5 名 | `the 8`、`to 4`、`is 3`、`not 3`、`value 3` |
   | 最長的字 | `compiler` |
   | 長度 ≥ 4 的字數 | `39` |

## 實驗

每個都先預測，再試。預測和結果直接寫在各實驗下面。

### 實驗 A

寫這一行，後面不接任何東西，預測畫面會印出什麼；然後在尾端加上 `.count()` 再跑一次：

```rust
words.iter().map(|w| { println!("看到 {w}"); w.len() });
```

**預測**：

**結果**：

### 實驗 B

不排序，直接 `for (word, n) in &map` 印出前 5 筆。連續執行程式三次，比較順序

**預測**：

**結果**：

### 實驗 C

把 `HashMap<String, usize>` 改成 `HashMap<&str, usize>`，key 直接用從 `TEXT` 切出來的 `&str`（先不轉小寫）。編得過之後，再試著把 `to_lowercase()` 的結果當 key 放進去

**預測**：

**結果**：

## 問題

回答直接寫在各題下面。

### 問題 1

實驗 A：沒有 `.count()` 時為什麼什麼都沒印？iterator 鏈是誰讓它動起來的？`map` 自己會主動做事嗎？

**回答**：

### 問題 2

實驗 B：順序為什麼每次不同？那個「不同」的來源是哪一層提供的？為什麼 `top_n` 一定要排序？

**回答**：

### 問題 3

第 6 步的 closure 抓住了 `min_len`。closure 在記憶體裡是什麼東西？它存的是 `min_len` 的值，還是指向 `min_len` 的位址？

**回答**：

### 問題 4

實驗 C：`&str` 當 key 時，不轉小寫可以、轉小寫不行。差別在於那些字元住在哪裡、活多久？

**回答**：

### 問題 5

`count_loop` 和 `count_iter` 編譯成機器碼之後，你預測哪個比較快？為什麼？

**回答**：

## 交卷

commit 之後說「複盤 q06」。
