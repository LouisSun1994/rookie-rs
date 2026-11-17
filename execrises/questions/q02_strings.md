# q02：字串的三種簽名

檔案：[`src/bin/q02_strings.rs`](../src/bin/q02_strings.rs)

## 背景

同一個字串，函式可以用三種方式收：拿走（`String`）、借來看（`&str`）、借來改（`&mut String`）。
這題把三種都寫一次，並且印出位址，驗證 heap 上的字元到底有沒有被複製。

## 要做的事

1. `fn count_vowels(s: &str) -> usize`：回傳母音（a e i o u，不分大小寫）的個數。
2. `fn first_word(s: &str) -> &str`：回傳第一個空白之前的部分；沒有空白就回傳整個字串。
3. `fn append_bang(s: &mut String)`：在尾端加一個 `!`。
4. `fn shout(s: String) -> String`：轉成大寫、尾端再加一個 `!`，回傳。
5. `main` 依序做下面的事並印出結果：

   | 步驟 | 預期 |
   |---|---|
   | `let mut name = String::from("hello ferris");` | |
   | `count_vowels(&name)` | `4` |
   | `first_word(&name)` | `hello` |
   | `append_bang(&mut name)` 後印 `name` | `hello ferris!` |
   | `let loud = shout(name);` 印 `loud` | `HELLO FERRIS!!` |
   | `first_word("no_space")` | `no_space` |
   | `count_vowels("AEIOU xyz")` | `5` |

6. 位址觀察：四個函式的第一行都印出收到的字串的 `s.as_ptr()`（用 `{:p}`）；
   `main` 在每次呼叫前也印一次 `name.as_ptr()`，`shout` 回傳後印 `loud.as_ptr()`。

## 限制

- 全檔不准出現 `.clone()`，實驗 D 除外
- 只讀不改的參數一律寫 `&str`，不要寫 `&String`

## 實驗

四個實驗都是「在 `main` 裡暫時加幾行程式，看編譯器或執行結果怎麼反應」。

做法：先在註解寫下預測 → 加上那幾行 → 編譯或執行 → 記錄結果 → 把那幾行註解掉留在檔案裡，再做下一個。

### 實驗 A：字串交給 `shout` 之後，還能不能用

加在 `main` 的最後面（`shout(name)` 之後）：

```rust
let loud = shout(name);   // 這行是原本就有的
println!("{name}");       // ← 加這一行
```

預測：編不編得過？

### 實驗 B：借出去看的同時，拿去改

加在 `shout(name)` **之前**（這時 `name` 還在）：

```rust
let w = first_word(&name);   // w 借用了 name 的一部分
append_bang(&mut name);      // 修改 name
println!("{w}");             // 修改之後，還在使用 w
```

預測：編不編得過？

### 實驗 C：和 B 一樣，但修改之後不再使用 `w`

把實驗 B 的第三行刪掉，只留前兩行：

```rust
let w = first_word(&name);   // w 借用了 name 的一部分
append_bang(&mut name);      // 修改 name
                             // 之後沒有任何地方用到 w
```

預測：編不編得過？結果和 B 一樣嗎？

（編譯器可能會警告 `w` 沒被使用，那是警告不是錯誤，不影響這個實驗。）

### 實驗 D：`.clone()` 到底做了什麼

加在 `shout(name)` **之前**：

```rust
let copy = name.clone();
println!("name: {:p}", name.as_ptr());
println!("copy: {:p}", copy.as_ptr());
```

預測：兩個位址相同還是不同？

## 在註解裡回答（先寫預測，再跑）

1. 第 6 步印出的位址，哪些相同、哪些不同？`&str`、`&mut String`、`String` 三種傳法，各自讓 heap 上的字元被複製了幾次？
2. 實驗 B：假設編譯器放行，執行期可能出什麼事？（提示：`push` 在容量不夠時會做什麼、`w` 指向哪裡）
3. 實驗 B 和 C 只差一行 `println!`。兩者的結果相同嗎？如果不同，編譯器判斷的依據是什麼？
4. `first_word("no_space")` 傳進去的是字串常值。它的字元住在哪裡（stack / heap / 其他）？誰負責釋放、什麼時候？
5. `&name` 的型別是 `&String`，為什麼能傳給要 `&str` 的函式？

## 交卷

commit 之後說「複盤 q02」。
