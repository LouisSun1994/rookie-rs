# Experience 04 — Rust 的行話：複盤和 rustc 訊息裡的詞各指什麼

> 經驗不是筆記：起點是一個實際撞到的問題，往下挖到能解釋它為止。這一篇撞到的不是程式跑不動，是**讀不懂**：複盤裡的 variant、arm、clippy，rustc 訊息裡的 constructed、moved，一個詞卡住，整段就讀不下去。把這些詞依「屬於哪一層」排開，每個詞給一句話和一個自己寫過的例子。
> 當時定位：🐣 入門，動手寫到 q05（`Result`、`?`、自訂 error）。
> 起因：q05 複盤的一句「六個 variant 全都由 `expr` 的內容決定」看不懂，回頭發現 variant、field、arm 這幾個詞一直在用，卻沒有人定義過。
> 接在哪裡：[note 01 §1](../notes/01-memory-model-async-runtime-os.md)（語法速記）、[note 01 §4](../notes/01-memory-model-async-runtime-os.md)（copy / move / clone / borrow）、[note 01 §6](../notes/01-memory-model-async-runtime-os.md)（enum 的 tag、niche）、[note 04](../notes/04-borrow-exclusivity-lifetime-elf.md)（借用規則、lifetime）、[經驗 03 §11–§12](03-game-engine-ecs-bevy.md)（toolchain、crate、feature、ABI）。這些地方講過機制的，這裡只給詞和一句話，不重寫。
> 來源與驗證狀態：例子取自 q03–q05 自己寫的程式碼。§8 的錯誤代碼與訊息原文在 rustc 1.97.0 上實際編譯確認過（E0004、E0117、E0308、E0390、E0507，以及 `never constructed` / `never read` 兩條 warning）；E0382、E0502、E0597、E0277 的訊息在 q04、q05 的複盤裡實際撞到過。

---

## 0. 一句話版本

Rust 的詞大多在回答同一個問題：**這個東西在程式碼的哪個位置、扮演什麼角色**。同一串字（`Op::Add`）放在不同位置就是不同的東西（做出一個值，或是比對一個值），所以先認位置，再認詞。讀 rustc 訊息時，先找動詞（moved、borrowed、constructed、expected / found），它說的就是哪個動作出了問題。

## 1. 事件：發生了什麼

卡住的不是某一行程式碼，是用來討論程式碼的詞。

```
時間   在做什麼            卡住的詞                       當時的理解
q04   讀 rustc warning    variant is never constructed   「constructed」是要實作什麼；parse 回傳 Option<Command>，所以要滿足所有 enum
q04   讀 rustc error      borrow of moved value          這是 borrow 的問題，「在哪裡被借了」
q04   回答問題            （自己用的詞）                  把 variant 叫成 struct、member
q05   讀導師的解說        arm                            一直出現，沒有人解釋過
q05   回答問題            （自己用的詞）                  把 ? 叫成 optional、把 panic 叫成警告、把結束代碼叫成 signal
q05   讀複盤              clippy                         不知道是什麼
q05   讀複盤              variant、expr                  variant 是 enum member 的意思嗎；expr 是什麼
q05   讀 rustc error      required for … to implement    把 Termination 讀成 Terminal
                          Termination
```

共同點：每一次都是**詞先卡住**，機制其實不難。而且有一半是導師的問題：直接拿術語（arm、clippy）或程式裡的參數名（`expr`）來用，沒有先定義。

## 2. 分層模型（骨架）

每個詞屬於一層。讀到不認識的詞，先問它是哪一層的。

| 層 | 這一層的詞在講什麼 | 例 | 本文 |
|---|---|---|---|
| 語法 | 程式碼的零件：這段字是什麼 | expression、pattern、arm、item | §3 |
| 型別 | 資料的形狀與能力 | field、variant、trait、impl、method | §4 |
| 擁有權 | 值歸誰、能不能再用 | move、borrow、drop、lifetime | §5 |
| 錯誤處理 | 失敗怎麼表示、怎麼往上傳 | propagate、unwrap、panic | §6 |
| 工具鏈 | 誰在處理你的程式碼 | crate、cargo、clippy、lint | §7 |
| rustc 訊息 | 編譯器怎麼描述問題 | expected / found、constructed、diverge | §8 |
| 評語 | 人在評論寫法 | idiomatic、desugar、boilerplate | §9 |

## 3. 語法層：程式碼的零件

**核心規則**：Rust 的程式碼由 item 組成，item 裡面是 statement 和 expression。expression 會算出一個值，statement 不會。

| 詞 | 中文 | 一句話 | 例 |
|---|---|---|---|
| item | 項目 | 寫在最外層的定義 | `fn`、`struct`、`enum`、`impl`、`use`、`const` |
| expression | 運算式 | 會算出一個值的一段程式碼 | `1 + 2`、`eval(li)`、整個 `match`、整個 `if` |
| statement | 陳述式 | 做一件事，不產生值 | `let x = 1;`，或 expression 後面加 `;` |
| block | 區塊 | `{ … }`，本身也是 expression，值是最後一行 | `{ let a = 1; a + 1 }` 的值是 2 |
| tail expression | 尾運算式 | block 最後一行、沒加 `;` 的那個 expression | `Ok((lhs, rhs))` |
| literal | 字面值 | 直接寫在程式碼裡的值 | `42`、`"hi"`、`true` |
| binding | 綁定 | 把一個名字接到一個值上，平常說的「變數」 | `let v = …` 的 `v` |
| pattern | 模式 | 描述「值長什麼樣」，用來比對並拆出裡面的東西 | `Some(op)`、`(lhs, rhs)`、`["add", item, count]` |
| arm | 分支 | `match` 裡的一條 `pattern => expression` | `Op::Add => lhs.checked_add(rhs)…` |
| guard | 守衛 | arm 的 pattern 後面多加的 `if 條件` | `Some(n) if n > 0 => …` |
| scrutinee | 被比對的值 | `match` 後面那個 expression | `match token { … }` 的 `token` |
| wildcard | 萬用 | `_`，比對任何值、不綁名字 | `_ => None` |
| path | 路徑 | 用 `::` 串起來的名字 | `Op::Add`、`std::fmt::Display` |
| turbofish | （不翻） | `::<T>`，手動指定泛型的型別 | `token.parse::<i64>()` |
| attribute | 屬性 | `#[…]`，貼在 item 上給編譯器看的標記 | `#[derive(Debug)]`、`#[allow(dead_code)]` |
| macro | 巨集 | 名字後面有 `!`，編譯期展開成別的程式碼 | `println!`、`vec!`、`write!` |
| shadowing | 遮蔽 | 用同一個名字再 `let` 一次，舊的被蓋住 | `let s = s.trim();` |
| diverge | 發散 | 這段程式碼不會「走完」，一定會離開 | `return`、`continue`、`break`、`panic!` |

**關鍵細節**

- **同一串字，位置決定它是什麼。** `Command::Add(name, qty)` 寫在 `=` 右邊或當參數，是 expression：做出一個值。寫在 `match` 的 arm 左邊或 `let` 左邊，是 pattern：比對一個值並拆開。它從來不是函式呼叫。
- pattern 出現的位置不只 `match`：`let (a, b) = …`、`if let Some(x) = …`、`let Some(x) = … else { … }`、函式參數、`for (i, x) in …` 都是。
- `let … else` 的 `else` 必須 diverge。rustc 的訊息就用這個字：`` `else` clause of `let...else` does not diverge ``。
- `expr` 不是術語。它是 expression 的縮寫，常被拿來當變數名（q05 的 `fn eval(expr: &str)`）。文件裡看到 `expr` 通常是指「某個運算式」。

## 4. 型別層：資料的形狀與能力

**核心規則**：struct 的成員叫 field，一個值**同時擁有全部**。enum 的成員叫 variant，一個值**只會是其中一個**。

| | struct | enum |
|---|---|---|
| 成員叫什麼 | field（欄位） | variant（變體） |
| 一個值裡有幾個 | 全部 | 其中一個 |
| 例 | `Player { name, hp }` | `CalcError::Empty` 或 `CalcError::BadToken(…)` |
| 取用 | `p.name` | `match` 拆開 |

variant 有三種寫法：

| 寫法 | 叫什麼 | 例 |
|---|---|---|
| 只有名字 | unit variant | `Empty`、`Op::Add` |
| 名字加括號 | tuple variant | `BadToken(String)` |
| 名字加大括號 | struct variant | `Move { x: i32, y: i32 }` |

variant 括號裡帶的資料常被叫 payload（承載的資料）。

| 詞 | 中文 | 一句話 | 例 |
|---|---|---|---|
| primitive | 基本型別 | 語言內建的型別 | `i64`、`bool`、`char`、`&str` |
| unit | 單元 | `()`，只有一個值的型別，代表「沒有東西」 | `fn main()` 的回傳型別 |
| generic | 泛型 | 型別或函式留一個洞，用的時候再填 | `Vec<T>`、`Result<T, E>` |
| type parameter | 型別參數 | 那個洞的名字 | `Result<T, E>` 的 `T`、`E` |
| trait | 特徵 | 一份能力的規格：「實作我的型別要有這些 method」 | `Display`、`Debug`、`Drop` |
| trait bound | 特徵約束 | 對型別參數的要求：「`E` 必須有 `Debug`」 | `E: Debug` |
| impl | 實作 | 幫型別加上 method | 見下表 |
| method | 方法 | 第一個參數是 `self` 的函式，用點呼叫 | `op.calc(…)` |
| associated function | 關聯函式 | 寫在 `impl` 裡、沒有 `self` 的函式，用 `::` 呼叫 | `Op::parse(token)`、`String::from("a")` |
| receiver | 接收者 | method 的第一個參數 | `self`、`&self`、`&mut self` |
| signature | 簽名 | 函式的名字、參數型別、回傳型別，不含 body | `fn eval(expr: &str) -> Result<i64, CalcError>` |
| derive | 衍生 | 請編譯器照欄位自動寫出 trait 的實作 | `#[derive(Debug, Clone)]` |
| marker trait | 標記特徵 | 沒有任何 method 的 trait，只當標籤 | `Copy`、`Send`、`Sync` |
| prelude | 預先匯入 | 不用 `use` 就能用的那批名字 | `Option`、`Result`、`Vec`、`String` |

`impl` 有兩種，長得像但意思不同：

| 寫法 | 叫什麼 | 意思 | 例 |
|---|---|---|---|
| `impl 型別 { … }` | inherent impl（固有實作） | 幫**我自己的型別**加 method | `impl Op { fn parse… }` |
| `impl Trait for 型別 { … }` | trait impl（特徵實作） | 讓這個型別符合某份規格 | `impl fmt::Display for CalcError { … }` |

**關鍵細節**

- **orphan rule（孤兒規則）**：`impl Trait for 型別` 時，trait 和型別至少要有一個是你自己的 crate 定義的。inherent impl 則只能寫在自己的型別上。`impl i64 { … }`、`impl Drop for u32`、`impl Display for Result<…>`、`impl Termination for Result<…>` 四次撞到的都是這一條。
- 嚴格說 method 也是 associated function 的一種。平常說 associated function 多半是指沒有 `self` 的那種。
- prelude 裡的名字可以被自己的 `use` 蓋掉。q05 的 `use std::fmt::Result;` 就把兩個參數的 `Result<T, E>` 蓋成了 `fmt::Result`。
- enum 的 tag、discriminant（記「現在是哪個 variant」的那格）、niche 在 note 01 §6。

## 5. 擁有權層：值歸誰、能不能再用

機制在 note 01 §4 與 note 04，這裡只對詞。

| 詞 | 中文 | 一句話 |
|---|---|---|
| owner | 擁有者 | 負責釋放這個值的那個變數（或欄位、容器的格子） |
| move | 搬移 | 以值使用一個不是 `Copy` 的東西：擁有權交出去，原變數失效 |
| copy | 複製 | 以值使用一個 `Copy` 的東西：多一份，原變數照樣能用 |
| clone | 克隆 | 明確呼叫 `.clone()`，由型別作者寫的程式碼做出第二份 |
| borrow | 借用 | 做出一個 reference，擁有權不變 |
| reference | 參考 | `&T` 或 `&mut T`，一個位址加上編譯期的使用規則 |
| shared reference | 共享參考 | `&T`，可以同時有很多個，只能讀 |
| mutable reference | 可變參考 | `&mut T`，同時只能有一個，可以改。也叫 exclusive reference |
| dereference（deref） | 解參考 | `*r`，從 reference 走到它指的那個位置 |
| place | 位置 | 記憶體裡的一個地方：變數、`*r`、`s.field`、`v[0]` |
| value | 值 | 放在位置裡的內容 |
| scope | 作用域 | 一個名字有效的那段程式碼，通常到 `}` 為止 |
| drop | 釋放 | 值的擁有者離開 scope 時，rustc 插入的清理動作 |
| lifetime | 壽命 | 一個 reference 可以被使用的那段程式碼範圍。寫成 `'a` |
| temporary | 暫時值 | 沒有被綁到名字上的中間結果，所在的那個 statement 結束就消失 |
| dangling | 懸空 | reference 指的東西已經不在了。Rust 在編譯期擋掉 |

**關鍵細節**

- place 和 value 的區別解釋了 `*self`：它是「借條指的那個位置」，寫 `*` 不搬任何東西。會不會 move 要看接下來對這個位置做什麼。
- 訊息裡的 `behind a shared reference` 意思是「這個位置是透過 `&` 才到得了的」，所以不能從那裡搬東西走。

## 6. 錯誤處理層

| 詞 | 中文 | 一句話 | 例 |
|---|---|---|---|
| `Option<T>` | | 有或沒有 | `Some(v)` / `None` |
| `Result<T, E>` | | 成功或失敗，失敗帶原因 | `Ok(v)` / `Err(e)` |
| unwrap | 拆開 | 把殼剝掉拿裡面的值；殼是 `None` / `Err` 就 panic | `.unwrap()`、`.expect("…")` |
| propagate | 往上傳 | 自己不處理，把 `Err` 交給呼叫者 | `?` |
| early return | 提早回傳 | 函式還沒走到底就 `return` | `?` 在 `Err` 時做的事 |
| panic | 恐慌 | 執行期發現不該發生的狀態，這條 thread 停下 | `panic!`、`unreachable!`、陣列越界 |
| unwind | 回溯 | panic 之後沿著呼叫鏈一層層往回走，沿路照樣 drop | |
| exhaustive | 窮舉 | `match` 把所有可能都列到了 | 不寫 `_`、每個 variant 一條 arm |
| recoverable / unrecoverable | 可恢復 / 不可恢復 | 前者用 `Result`，後者 panic | |

**關鍵細節**

- `?` 不是別的語言的 optional chaining（`?.`）。它是「`Ok` 就拆出值，`Err` 就 `return Err`」。
- panic 不是 warning。warning 是編譯期 rustc 印的，程式照樣能跑。panic 是執行期，行程以結束代碼 101 結束。
- `Ok`、`Err`、`Some`、`None` 都是 variant，不是關鍵字，也不是函式。

## 7. 工具鏈層

三個 process 怎麼接力、crate / feature / ABI 的機制在經驗 03 §11–§12。

| 詞 | 一句話 |
|---|---|
| rustc | 編譯器。把一個 crate 的原始碼變成機械碼 |
| cargo | 建置工具兼套件管理。讀 `Cargo.toml`，決定用什麼參數呼叫 rustc 幾次 |
| rustup | 管理 toolchain 的版本（裝、切換、更新） |
| toolchain | 一整組同版本的工具：rustc、cargo、std、clippy、rustfmt |
| crate | 編譯單位。rustc 一次編一個 crate |
| package | 一個 `Cargo.toml` 管的範圍，裡面可以有好幾個 crate |
| binary / library | crate 的兩種：有 `main` 可以執行，或只給別人用 |
| module | crate 內部用 `mod` 分的命名空間 |
| dependency | 你的 crate 用到的別人的 crate |
| profile | 一組編譯設定。`dev`（`cargo build`）和 `release`（`--release`） |
| target | 編給哪種機器和系統，例如 `aarch64-apple-darwin` |
| edition | 語言的版次（2021、2024）。同一個編譯器支援多個 edition |
| lint | 一條檢查規則，抓「能編但可疑」的寫法 |
| warning / error | lint 或編譯器的回報。warning 還是編得過，error 編不過 |
| clippy | 官方的 linter。在 rustc 的分析結果上多跑幾百條 lint。`cargo clippy` |
| rustfmt | 排版工具，只改格式不看內容。`cargo fmt` |
| `cargo check` | 只跑到型別和借用檢查，不產生機械碼，比 build 快 |
| error code | `E0507` 這種編號。`rustc --explain E0507` 可以看完整說明 |

## 8. rustc 訊息裡的詞

**核心規則**：先找動詞。訊息的動詞就是出問題的那個動作。

| 訊息（原文） | 代碼 | 白話 | 注意 |
|---|---|---|---|
| `mismatched types: expected X, found Y` | E0308 | 這個位置要 X，你給的是 Y | expected 是位置要的，found 是你寫的 |
| `borrow of moved value` / `use of moved value` | E0382 | 這個變數的值已經被搬走了，你又去用它 | 重點是 **moved**，不是 borrow。要找的是「在哪裡被搬走」 |
| `cannot move out of … which is behind a shared reference` | E0507 | 想從借來的東西裡搬走不是 `Copy` 的值 | note 01 §4 |
| `cannot borrow … as mutable because it is also borrowed as immutable` | E0502 | 還有 `&` 在用的時候又要 `&mut` | 多讀 XOR 單寫，note 04 §4 |
| `… does not live long enough` | E0597 | 被借的東西比借它的 reference 早消失 | 要找的是擁有者活到哪 |
| `non-exhaustive patterns: … not covered` | E0004 | `match` 沒有列完，訊息會告訴你漏了哪個 | |
| `the trait bound … is not satisfied` / `… doesn't implement …` | E0277 | 這裡要求型別有某個 trait，它沒有 | 常見解法是加 `#[derive(…)]` |
| `required for … to implement …` | （E0277 的 note） | 上面那條的推理過程：「因為 A 要有 X，所以 B 得先有 Y」 | q05：`Result<(), CalcError>` 要有 `Termination`，所以 `CalcError` 要有 `Debug` |
| `only traits defined in the current crate can be implemented for types defined outside of the crate` | E0117 | orphan rule：trait 和型別都不是你的 | §4 |
| `cannot define inherent impl for primitive types` | E0390 | 不能 `impl i64 { … }` | §4 |
| `` `else` clause of `let...else` does not diverge `` | E0308 | `else` 裡沒有 `return` / `continue` / `break` / panic | §3 |
| `variant … is never constructed` | warning | 這個 variant 定義了，但沒有任何會被執行到的地方做出它的值 | constructed = 做出一個值，不是「實作」 |
| `field … is never read` | warning | 這個欄位有寫進去，但沒有人讀 | |
| `unused variable` / `function … is never used` | warning | 定義了沒用到 | 變數名前面加 `_` 可以關掉 |
| `variable does not need to be mutable` | warning | 寫了 `mut` 但沒有改它 | |

訊息的結構：

| 開頭 | 是什麼 |
|---|---|
| `error[E…]:` / `warning:` | 問題本身 |
| `-->` | 檔案、行、欄 |
| `^^^^` 和 `----` | 指著出問題的那段程式碼，旁邊的字是針對那一段的說明 |
| `= note:` | 補充的背景或推理過程 |
| `help:` | rustc 建議的改法，通常可以直接照做 |

## 9. 評語裡的詞

讀文件、複盤、別人的 review 會遇到的。

| 詞 | 意思 | 例 |
|---|---|---|
| idiomatic | 道地的。符合這個語言社群習慣的寫法 | 用 `?` 而不是手寫 `match … Err(e) => return Err(e)` |
| syntactic sugar | 語法糖。比較短的寫法，意思和長的一樣 | `?`、`for` 迴圈、`self.name`（自動補 `*`） |
| desugar / expand | 把語法糖還原成長的寫法 | `?` 展開成 `match` |
| boilerplate | 樣板。每次都要寫、但沒什麼資訊量的程式碼 | 手寫 `Display` 的 `match self { … }` |
| ergonomic | 好寫、順手 | `match self` 自動把綁定變成 reference，叫 match ergonomics |
| zero-cost | 零成本。抽象不會讓編出來的機械碼變慢 | iterator 和手寫迴圈一樣快 |
| sound / unsound | 健全 / 不健全。安全的程式碼有沒有可能造成未定義行為 | |
| UB（undefined behavior） | 未定義行為。語言不保證會發生什麼 | 正常情況下要用到 `unsafe` 才可能 |
| invariant | 不變量。程式裡「永遠應該成立」的條件 | q05：每處理完一個運算子，stack 頂端一定是它的結果 |

## 10. 解法對照：遇到不認識的詞怎麼辦

| 情況 | 做法 | 採用 |
|---|---|---|
| rustc 訊息看不懂 | `rustc --explain E0xxx`；先找動詞（§8） | ✅ |
| clippy 的 warning 看不懂 | 每條 warning 附的網址，裡面有這條 lint 的理由和例子 | ✅ |
| 型別或 method 是什麼 | std 文件，搜尋型別名 | ✅ |
| 語法名詞（pattern、place、item） | The Rust Reference，裡面有 Glossary | ✅ |
| 導師或複盤用了沒解釋過的詞 | 直接問「這個詞是什麼」。這是導師該先定義的 | ✅ |
| 先跳過，看上下文猜 | 會把別的語言的意思帶進來（`?` 當成 optional） | ❌ |

## 11. 陷阱清單

每條都是實際講錯或讀錯過的。

1. ❌ variant 就是 struct（或 member） → ✅ variant 是 enum 的成員，一個值只會是其中一個；struct 的成員叫 field，一個值全部都有 → 為什麼：`Command` 是一個型別，四個 variant 是這個型別的值的四種形狀。
2. ❌ `match` arm 左邊的 `Command::Add(name, qty)` 是在呼叫函式 → ✅ 那是 pattern，在比對並拆開；同一串字寫在 `=` 右邊才是做出一個值 → 為什麼：位置決定它是 pattern 還是 expression。
3. ❌ `variant is never constructed` 是說要「實作」這個 variant → ✅ constructed 是「做出這個 variant 的值」，warning 是說沒有任何會執行到的地方做出它 → 為什麼：定義一個回傳它的 fn 不算，那個 fn 要有人呼叫才算。
4. ❌ `borrow of moved value` 是 borrow 出了問題 → ✅ 是值已經被 move 走了，之後又去用它 → 為什麼：訊息的重點在 moved，要找的是哪一行把它搬走（常藏在 `match` 的 pattern 裡）。
5. ❌ `?` 是 optional → ✅ `?` 是 propagate：`Ok` 拆出值，`Err` 就 `return Err` → 為什麼：別的語言的 `?.` 是「沒有就略過」，Rust 的 `?` 是「失敗就離開函式」。
6. ❌ debug 版溢位是 overflow「警告」 → ✅ 是 panic → 為什麼：warning 在編譯期、程式照跑；panic 在執行期、thread 停下。
7. ❌ process 結束交出去的是 signal → ✅ 是 exit status（結束代碼） → 為什麼：signal 是 kernel 送給 process 的通知，是另一套機制。
8. ❌ std 的 `impl u8 { fn checked_add … }` 是 override → ✅ 是 inherent impl，std 幫自己的型別加 method → 為什麼：Rust 沒有繼承，沒有東西可以 override。
9. ❌ `Termination` 讀成 Terminal → ✅ Termination 是「終止」，管 `main` 的回傳值怎麼變成結束代碼 → 為什麼：和終端機無關。
10. ❌ `Copy` 是一個 method → ✅ 是 marker trait，沒有任何 method → 為什麼：複製動作是 rustc 產生的，trait 只是標籤。
11. ❌ `expr` 是 Rust 的術語 → ✅ 是 expression 的縮寫，在 q05 只是參數的名字 → 為什麼：變數名和術語撞字。
12. ❌ enum 把 variant「映射」到 fn → ✅ variant 是值，由一個 fn 裡的 `match` 分流 → 為什麼：variant 本身不帶函式，漏了一條 arm 是編譯期被擋，不是執行期查不到。
