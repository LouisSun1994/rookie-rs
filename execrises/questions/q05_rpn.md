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

每個都先預測，再試。預測和結果直接寫在各實驗下面。

### 實驗 A

把 `checked_add` 換回普通的 `+`，分別用 debug 和 `--release` 跑 `9223372036854775807 1 +` 那一行。看完換回來

**預測**：

**結果**：

```
debug 會顯示 overflow警告
-- thread 'main' (443131) panicked at src/bin/q05_rpn.rs:69:40:
-- attempt to add with overflow
-- note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
release 會通過但數值很怪
-- 9223372036854775807 1 + => -9223372036854775808
```

### 實驗 B

挑一處 `?`，改寫成功能完全相同的 `match`，確認輸出不變後改回來

**預測**：
```=rust
    "+" => {
                // num_stack.push(augend.checked_add(addend).ok_or(CalcError::Overflow)?);
                let Some(sum) = augend.checked_add(addend) else {
                    return Err(CalcError::Overflow);
                };
                num_stack.push(sum);
            }
```

**結果**：

### 實驗 C

把 `main` 改成 `fn main() -> Result<(), CalcError>`，在裡面對一個會失敗的運算式寫 `eval("1 +")?;`。觀察畫面印出什麼，以及程式結束後 `echo $?` 的數字。（需要 `#[derive(Debug)]`）

**預測**：

```=rust
fn main() -> Result<(), CalcError> {
    // 3 4 +	7
    // 5 1 2 + 4 * + 3 -	14
    // 2 3 4 * +	14
    // 7 2 /	3
    // 10 3 -	7
    // 1 +	StackUnderflow
    // 4 0 /	DivideByZero
    // 1 2	TooManyOperands(2)
    // （空字串）	Empty
    // 2 x +	BadToken("x")
    // 9223372036854775807 1 +	Overflow
    // -9223372036854775808 -1 /	Overflow

    let list = vec![
        "3 4 +",
        "5 1 2 + 4 * + 3 -",
        "2 3 4 * +",
        "7 2 /",
        "10 3 -",
        "1 +",
        "4 0 /",
        "1 2",
        "",
        "2 x +",
        "9223372036854775807 1 +",
        "-9223372036854775808 -1 /",
        "1 +",
    ];
    for li in list {
        match eval(li) {
            Ok(v) => println!("{li} => {v}"),
            Err(e) => {
                return Err(e);
                // let question = if li.is_empty() { "(空字串)" } else { li };
                // println!("{question} => {e}")
            }
        };
    }
    Ok(())
}
```

```=bash
cargo q5 && echo $?

Error: StackUnderflow
```

**結果**：

## 問題

回答直接寫在各題下面。

### 問題 1

`?` 做了哪幾件事？用實驗 B 你寫出來的 `match` 說明。

**回答**：

```
這optional會把錯誤反饋出去，如果是真處理Ok()是假處理Err()。
```

### 問題 2

用了 `checked_*` 之後，debug 和 release 的行為還會不同嗎？和 q01 的溢位檢查相比，這次的檢查是誰寫的、在哪個時期執行？

**回答**：

```
debug 會檢查overflow，但是是runtime才顯示。
release 不會處理overflow警告。
```

### 問題 3

實驗 C 的那個結束代碼是誰設定的？從「`main` 回傳 `Err`」到「shell 的 `$?` 拿到數字」，中間經過哪幾層？

**回答**：

```
猜想一:
fn main() -> Result<(), CalcError>
Result 中 
Ok() 反饋stdout
Err() 反饋stderr

猜想二:
cargo q5 && echo $?
process結束的signal就是回傳值?(猜測，但這跟我上面猜測反例。)
```

### 問題 4

這題裡哪些情況適合用 `Result`？有沒有哪種情況你認為直接 panic 才合理？判斷標準是什麼？

**回答**：

```
允許的錯誤使用Result，異常的錯誤需要panic(panic是process中斷1)。
這題為例，我覺得都使用Result較佳，因為可以處理N個題目，單一題目錯誤不至於讓process中斷，且業務上也沒有關聯性。
```

### 問題 5

`BadToken(String)` 為什麼帶的是 `String` 而不是 `&str`？如果要改成 `&str`，`CalcError` 和 `eval` 的簽名會變成什麼樣子？

**回答**：

```
恩，我也想知道其中奧妙。

猜測：
這邊使用String希望作用是move，
往這方面去猜就是期望每個frame都可以正常釋放，
如果使用&str的話，
反而是某一次的frame內的&str一直binding，
但如果是使用String當錯誤拋到最外層時候，
內層frame也都釋放了。
```

## 加分

回到 q03，把 `pick_up` 的回傳型別從 `bool` 改成 `Result<(), Item>`：放不下時把物品還給呼叫端，而不是讓它消失。

```
改一下 pick_up -> pick_up_2
```

## 交卷

commit 之後說「複盤 q05」。
