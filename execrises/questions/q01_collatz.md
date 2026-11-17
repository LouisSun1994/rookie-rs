# q01：Collatz 步數

檔案：[`src/bin/q01_collatz.rs`](../src/bin/q01_collatz.rs)

## 規則

對一個正整數 n 重複做下面的事，直到 n 變成 1：

- n 是偶數 → `n / 2`
- n 是奇數 → `3n + 1`

例：`6 → 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1`，共 8 步。

## 要做的事

1. 寫 `fn collatz_len(n: u64) -> u32`，回傳從 n 走到 1 的步數。
   - `collatz_len(1) == 0`
   - `collatz_len(6) == 8`
2. `main` 印出 1 到 10 每個數的步數。
3. `main` 找出 1 到 999,999 之間步數最多的起點，印出起點和步數。
   - 自我驗證用的正解：起點 `837799`，`524` 步
4. 實驗：把 `collatz_len` 的參數型別改成 `u32`，分別跑 debug 和 `--release` 兩種 build，
   把兩次觀察到的現象寫在下面「實驗」那一節。
   - release 版如果一直跑不完就 Ctrl+C，「跑不完」本身也是一個觀察結果
   - 做完實驗把型別改回 `u64`

## 限制

- 只用 `loop` / `while` / `for i in a..b` 和 `if`，先不要用 iterator 方法（`.map`、`.max_by_key` 等）
- 至少有一處把 `if ... else ...` 當成 expression 用：`let x = if ... { ... } else { ... };`
- 不要寫 `return`，讓函式最後一個 expression 當回傳值

## 實驗

### 實驗：把參數型別改成 `u32`，debug 和 release 各跑一次

**debug**：

```
   Compiling execrises v0.1.0 (/Users/admin/rs/rookie-rs/execrises)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
     Running `target/debug/q01_collatz 3`
recursive 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
loop 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
```

**release**：

```
    Finished `release` profile [optimized] target(s) in 0.04s
     Running `target/release/q01_collatz 3`
recursive 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
loop 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
```

**複盤**：輸入 `3` 的路徑最大只到 16，沒有超出 `u32`，所以兩種 build 看不出差別。要選一個「路徑中途會超過 `u32` 上限（約 42.9 億）」的輸入，例如 `159487`：

| build | 結果 |
|---|---|
| debug | `panicked at ...: attempt to multiply with overflow`，在溢位的那一步立刻停下，並指出是哪一行 |
| release | 沒有任何錯誤，正常印出路徑，結尾是「共 245 步」。但正確答案（`u64`）是 183 步 |

release 的乘法結果被截斷成 32 bits，程式拿著錯的數字繼續算，剛好也走到 1，於是安靜地給出錯的答案。

release 的後果不固定，取決於截斷後的數字走去哪：

- 剛好走到 1 → 安靜的錯誤答案（上面這個例子）
- 掉進到不了 1 的循環 → `while` 版永遠跑不完；遞迴版 stack overflow（先前 `i32` + 遞迴版 + 輸入 `i32::MAX` 就是這種）

## 問題

### 問題 1

第 4 步兩種 build 的行為為什麼不同？那個「檢查」是誰放進去的：CPU、kernel、std，還是 rustc？在哪個時間點放的？

**預測**：rustc compile 檢查

**複盤**：「誰」答對了：rustc。要修的是「compile 檢查」這四個字，它把兩個不同的時間點合在一起了：

| 時期 | 誰 | 做什麼 |
|---|---|---|
| 編譯期 | rustc | 「放進」檢查：在乘法指令後面多產生幾條機器碼（看 CPU 的溢位旗標，有溢位就跳去 panic） |
| 執行期 | CPU | 「執行」那幾條機器碼，檢查才真的發生 |

證據：溢位與否取決於你從命令列輸入的數字，編譯當下 rustc 不可能知道，所以檢查不可能在編譯期完成，只能是編譯期預先埋好、執行期才跑。

兩種 build 的差別就是 rustc 有沒有埋那幾條指令：

```
dev profile     → overflow-checks = true  → 有埋 → 溢位時 panic
release profile → overflow-checks = false → 沒埋 → 溢位時截斷，繼續跑
```

這個設定由 cargo 依 profile 決定，再當成參數傳給 rustc。kernel 和 std 都沒有參與；CPU 只是照著機器碼執行。

### 問題 2

把第 3 步記錄最佳值的那個變數（例如 `let mut best = 0;`）的 `mut` 拿掉會怎樣？錯誤是在編譯期還是執行期出現？

**預測**：compile fail

**複盤**：對。錯誤訊息是：

```
error[E0384]: cannot assign twice to immutable variable `best`
```

這個檢查完全在編譯期完成（rustc 只看程式碼就能判斷，不需要知道輸入），編不過就沒有執行檔，執行期沒有任何對應的檢查或成本。

和問題 1 對照：

| 檢查 | 何時埋 / 何時查 | 出錯的形式 | 執行期成本 |
|---|---|---|---|
| `mut` 檢查 | 編譯期查完 | 編譯錯誤 | 無 |
| 溢位檢查 | 編譯期埋指令，執行期查 | 執行期 panic | 少量（所以 release 預設關掉） |

## 交卷

commit 之後說「複盤 q01」。
