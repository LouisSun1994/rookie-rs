# q05：RPN 計算機

檔案：[`src/bin/q05_rpn.rs`](../src/bin/q05_rpn.rs)

## 背景

RPN（Reverse Polish Notation，後序表示法）把運算子寫在運算元後面：`3 4 +` 就是 3 + 4。
計算方式是用一個 stack：讀到數字就 push；讀到運算子就 pop 出兩個數字、算完再 push 回去。

這題的重點不是演算法，是**所有會失敗的地方都用 `Result` 回報，不准 panic**。

## 資料

```rust
enum CalcError {
    Empty,                  // 運算式裡沒有任何東西
    BadToken(String),       // 不是數字也不是運算子，帶著那個 token
    StackUnderflow,         // 運算子需要兩個數字，但 stack 裡不夠
    DivideByZero,
    Overflow,               // 運算結果超出 i64
    TooManyOperands(usize), // 算完之後 stack 裡剩不只一個數字，帶著剩幾個
}
```

## 要做的事

1. `fn eval(expr: &str) -> Result<i64, CalcError>`：以空白分隔 token，支援 `+ - * /`（整數除法）。
2. `fn pop2(stack: &mut Vec<i64>) -> Result<(i64, i64), CalcError>`：pop 出兩個數字，不夠就回傳 `StackUnderflow`。`eval` 裡用 `?` 呼叫它。
3. 幫 `CalcError` 實作 `std::fmt::Display`，每個 variant 一句給人看的訊息。
4. `main` 對下面每個運算式印一行 `<運算式> => <結果或錯誤訊息>`：

   | 運算式 | 預期 |
   |---|---|
   | `3 4 +` | `7` |
   | `5 1 2 + 4 * + 3 -` | `14` |
   | `2 3 4 * +` | `14` |
   | `7 2 /` | `3` |
   | `10 3 -` | `7` |
   | `1 +` | `StackUnderflow` |
   | `4 0 /` | `DivideByZero` |
   | `1 2` | `TooManyOperands(2)` |
   | （空字串） | `Empty` |
   | `2 x +` | `BadToken("x")` |
   | `9223372036854775807 1 +` | `Overflow` |
   | `-9223372036854775808 -1 /` | `Overflow` |

## 限制

- `eval` 和 `pop2` 裡不准出現 `unwrap`、`expect`、`panic!`
- 四則運算不准直接用 `+ - * /`，改用 `checked_add` / `checked_sub` / `checked_mul` / `checked_div`
- `eval` 裡至少有三處用到 `?`

## 實驗

每個都先預測，再試。

- A. 把 `checked_add` 換回普通的 `+`，分別用 debug 和 `--release` 跑 `9223372036854775807 1 +` 那一行。看完換回來
- B. 挑一處 `?`，改寫成功能完全相同的 `match`，確認輸出不變後改回來
- C. 把 `main` 改成 `fn main() -> Result<(), CalcError>`，在裡面對一個會失敗的運算式寫 `eval("1 +")?;`。觀察畫面印出什麼，以及程式結束後 `echo $?` 的數字。（需要 `#[derive(Debug)]`）

## 在註解裡回答（先寫預測，再跑）

1. `?` 做了哪幾件事？用實驗 B 你寫出來的 `match` 說明。
2. 用了 `checked_*` 之後，debug 和 release 的行為還會不同嗎？和 q01 的溢位檢查相比，這次的檢查是誰寫的、在哪個時期執行？
3. 實驗 C 的那個結束代碼是誰設定的？從「`main` 回傳 `Err`」到「shell 的 `$?` 拿到數字」，中間經過哪幾層？
4. 這題裡哪些情況適合用 `Result`？有沒有哪種情況你認為直接 panic 才合理？判斷標準是什麼？
5. `BadToken(String)` 為什麼帶的是 `String` 而不是 `&str`？如果要改成 `&str`，`CalcError` 和 `eval` 的簽名會變成什麼樣子？

## 加分

回到 q03，把 `pick_up` 的回傳型別從 `bool` 改成 `Result<(), Item>`：放不下時把物品還給呼叫端，而不是讓它消失。

## 交卷

commit 之後說「複盤 q05」。
