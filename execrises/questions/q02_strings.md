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

做法：先寫下預測 → 加上那幾行 → 編譯或執行 → 記錄結果 → 把那幾行註解掉留在檔案裡，再做下一個。

### 實驗 A：字串交給 `shout` 之後，還能不能用

加在 `main` 的最後面（`shout(name)` 之後）：

```rust
let loud = shout(name);   // 這行是原本就有的
println!("{name}");       // ← 加這一行
```

**預測**：不行，因為已經被move

**複盤**：預測正確。

```
error[E0382]: borrow of moved value: `name`
```

`shout(name)` 把所有權交給了 `shout` 的參數 `s`，`main` 的 `name` 從那一行起不能再用。這是編譯期檢查；執行期 `name` 那三個欄位其實還留在 `main` 的 stack 上，只是編譯器不准你再碰它。

### 實驗 B：借出去看的同時，拿去改

加在 `shout(name)` **之前**（這時 `name` 還在）：

```rust
let w = first_word(&name);   // w 借用了 name 的一部分
append_bang(&mut name);      // 修改 name
println!("{w}");             // 修改之後，還在使用 w
```

**預測**：hello ferris!

**跑完後**：error[E0502]: cannot borrow `name` as mutable because it is also borrowed as immutable

**複盤**：預測錯在兩處。

1. 編不過。`w` 還要在第三行被使用，所以對 `name` 的唯讀借用一直持續到第三行；第二行想要 `&mut`（獨佔）借用，兩者重疊，編譯器拒絕。
2. 就算放行，`w` 也不會是 `hello ferris!`。`w` 是（起點位址, 長度 5），它不會跟著 `name` 變長，內容只會是 `hello`。

### 實驗 C：和 B 一樣，但修改之後不再使用 `w`

把實驗 B 的第三行刪掉，只留前兩行：

```rust
let w = first_word(&name);   // w 借用了 name 的一部分
append_bang(&mut name);      // 修改 name
                             // 之後沒有任何地方用到 w
```

（編譯器可能會警告 `w` 沒被使用，那是警告不是錯誤，不影響這個實驗。）

**跑完後**：看到這題，我猜測，可以compile是因為，w沒有被引用了，所以first_word borrow 釋放，因此append_bang PASS。

**複盤**：正確。修一個詞：「釋放」→「借用結束」。

借用持續到「最後一次使用」為止，不是到變數離開 scope 為止。`w` 在第一行之後沒有再被用到，借用在第一行就結束，第二行的 `&mut` 不衝突。借用是純編譯期的概念，執行期沒有任何東西被釋放。

### 實驗 D：`.clone()` 到底做了什麼

加在 `shout(name)` **之前**：

```rust
let copy = name.clone();
println!("name: {:p}", name.as_ptr());
println!("copy: {:p}", copy.as_ptr());
```

**預測**：copy 跟 name 是不同位址

**複盤**：預測正確，兩個位址不同。`clone` 向 allocator 要了一塊新的 heap 空間，把所有字元複製過去。這是「字元真的被複製」的操作，和傳參數不同（完整清單見問題 1）。

### 位址觀察（第 6 步印出來的位址）

| 位置 | 紀錄 |
|---|---|
| `main` 呼叫 `count_vowels` 前 | count_vowels 前後 位址一樣 (Heap) |
| `first_word` 裡 | first_word 竟然一樣，明明取空白前的那一串 (Heap) |
| `append_bang` 裡 | append_bang 一樣 (Heap) (提問:為什麼append_bang會變位址?) |
| `main` 呼叫 `shout` 前 | invoke時候就已經Copy新位址 |
| `shout` 裡 | 使用invoke的位址作調整，回傳(同上) |
| `loud`（`shout` 回傳後） | (同上) |

**複盤**：用 X、Y、Z 代表三個不同的位址，完整時間軸：

```
t0  String::from("hello ferris")   配置 12 bytes，從唯讀資料區
                                   複製字元進去，len=12 cap=12      位址 X
t1  count_vowels(&name)            借用，只傳位址                   位址 X
t2  first_word(&name)              借用；回傳的切片起點也是 X，長度 5
t3  append_bang → push             需要 13 bytes，cap 不夠
                                   → std 向 allocator 要更大的一塊
                                   → 原地能擴大就留在 X；
                                     不能就搬到新位置、釋放舊的     位址 Y（或仍是 X）
t4  shout(name)                    move：只搬 stack 上三個欄位      位址 Y
t5  shout 裡的 s                   和 t4 相同                       位址 Y
t6  s.to_ascii_uppercase()         配置一塊全新的，寫入大寫字元     位址 Z
t7  shout 結束                     rs 交給呼叫端；s 沒人要了，
                                   Y 那塊在這裡被釋放
t8  loud                           就是 rs                          位址 Z
```

逐格訂正：

- `first_word`「竟然一樣」：`&str` 是（起點位址, 長度）。`"hello"` 的起點就是整個字串的起點，所以位址相同，差別只在長度是 5。
- 「為什麼 `append_bang` 會變位址」：t3。和 `&mut` 無關，是 `push` 遇到容量不夠。會不會真的搬由 allocator 決定（macOS 上搬了，Linux 上同一份程式原地擴大）。
- 「invoke 時就已經 Copy 新位址」：錯。t4 的 move 沒有改變位址，證據是 `append_bang(inside)` 和 `shout(inside)` 兩行印出的位址相同。
- `loud` 的位址和 `shout` 裡的不同，原因是 t6 的 `to_ascii_uppercase` 產生了新字串，不是 return 造成的。

## 問題

### 問題 1

第 6 步印出的位址，哪些相同、哪些不同？`&str`、`&mut String`、`String` 三種傳法，各自讓 heap 上的字元被複製了幾次？

**預測**：&str,&mut String 相同;String不同(Copy new addr)

**複盤**：三種傳法，heap 上的字元被複製的次數都是 0。

| 傳法 | 實際傳了什麼 | heap 上的字元 |
|---|---|---|
| `&str` | （位址, 長度），16 bytes | 不動 |
| `&mut String` | 一個位址，8 bytes，指向 `main` stack 上的 `name` | 不動 |
| `String` | stack 上的三個欄位（ptr, len, cap），24 bytes | 不動，只是擁有者從 `main` 換成 `shout` |

這份程式裡字元真的被複製或搬動的有四處，都不是「傳參數」造成的：

- `String::from`（t0，從執行檔的唯讀資料區複製 12 bytes 到 heap）
- `push` 容量不夠（t3）
- `to_ascii_uppercase`（t6）
- `clone`（實驗 D）

### 問題 2

實驗 B：假設編譯器放行，執行期可能出什麼事？（提示：`push` 在容量不夠時會做什麼、`w` 指向哪裡）

**回答**：假設放行，w會亂掉。

**複盤**：方向對，精確的說法：t3 的 `push` 可能把內容搬到 Y 並「釋放」X 那塊，但 `w` 仍然是（X, 5），指著一塊已經還給 allocator 的記憶體。這叫 dangling pointer（懸空指標）。之後讀 `w` 可能讀到垃圾、讀到別人後來放進去的資料，或直接 crash，而且每次執行結果可能不同。

編譯器無法預知這次 `push` 會不會搬，所以只要 `w` 還會被用到，就一律不准改 `name`。

### 問題 3

實驗 B 和 C 只差一行 `println!`。兩者的結果相同嗎？如果不同，編譯器判斷的依據是什麼？

**回答**：我已經跑過，不一樣，w沒有被引用所以釋放，因此append可以borrow。

**複盤**：正確。依據是「`w` 之後還有沒有被使用」：

| 實驗 | `w` 的使用範圍 | 結果 |
|---|---|---|
| B | `append_bang` 之後還要用 | 唯讀借用跨過了 `&mut`，衝突 |
| C | `append_bang` 之前就用完了 | 兩個借用不重疊，通過 |

編譯器看的是每個借用「實際被使用的範圍」，不是變數的 scope。

### 問題 4

`first_word("no_space")` 傳進去的是字串常值。它的字元住在哪裡（stack / heap / 其他）？誰負責釋放、什麼時候？

**回答**："no_space" 這會在stack，main stack結束才釋放。

**複盤**：錯。字串常值的字元不在 stack 也不在 heap，是寫在「執行檔的唯讀資料區」。

| 東西 | 住在哪 |
|---|---|
| `"no_space"` 的 8 個字元 | 執行檔的唯讀資料區（編譯時 rustc 就寫進去了） |
| 指向它的（位址, 8） | stack，16 bytes |

程式啟動時 kernel 把執行檔載入記憶體，那些字元就已經在了，一直存在到 process 結束，沒有任何人釋放它。這就是它的型別 `&'static str` 裡 `'static` 的意思。

驗證：程式輸出裡，傳字串常值的那兩次呼叫，函式裡印出的位址和其他位址落在不同區段，而且兩個常值的位址彼此只差幾十 bytes。

### 問題 5

`&name` 的型別是 `&String`，為什麼能傳給要 `&str` 的函式？

**回答**：這也是我的好奇，我只能猜測compile有做手腳。

**複盤**：猜對了，機制叫 deref coercion（自動解參考轉換）：

| 層 | 做什麼 |
|---|---|
| std | `String` 實作了 `Deref<Target = str>`，宣告「我可以被當成 `str` 看」 |
| rustc | 編譯時發現手上是 `&String`、需要的是 `&str`，自動插入轉換 |
| 執行期 | 只是從 `String` 的三個欄位裡取出 ptr 和 len，沒有複製任何字元 |

反過來不行：`&str` 不能自動變成 `&String`。所以唯讀參數寫 `&str`，`String` 和字串常值兩種都能傳進來。

## 交卷

commit 之後說「複盤 q02」。
