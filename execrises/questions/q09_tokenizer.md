# q09：Tokenizer

檔案：[`src/bin/q09_tokenizer.rs`](../src/bin/q09_tokenizer.rs)

## 背景

tokenizer 把一串文字切成一個個 token（語彙單元）。這題要求**零複製**：token 不擁有自己的字元，只是指回原始字串的一段。
一個 struct 裡面放了 reference，就必須標 lifetime。這題的目的是弄清楚那個 `'a` 到底在描述誰。

## 資料

```rust
#[derive(Debug, PartialEq)]
enum Token<'a> {
    Num(i64),
    Ident(&'a str), // 識別字：英文字母或底線開頭，後面可接英文字母、數字、底線
    Op(char),       // 其他任何單一字元
    LParen,
    RParen,
}

struct Tokenizer<'a> {
    src: &'a str,
    pos: usize, // 目前讀到第幾個 byte
}
```

輸入只會有 ASCII 字元。

## 要做的事

1. `Tokenizer::new(src: &'a str) -> Tokenizer<'a>`
2. 實作 `Iterator`：
   ```rust
   impl<'a> Iterator for Tokenizer<'a> {
       type Item = Token<'a>;
       fn next(&mut self) -> Option<Token<'a>> { … }
   }
   ```
   跳過空白；讀完回傳 `None`。
3. `main` 對 `"price * (qty + 12) - tax2"` 做 tokenize，收集成 `Vec` 後用 `{:?}` 印出。預期：
   ```
   [Ident("price"), Op('*'), LParen, Ident("qty"), Op('+'), Num(12), RParen, Op('-'), Ident("tax2")]
   ```
4. 驗證零複製：對每個 `Ident`，印出它的 `as_ptr()` 減掉原始字串 `as_ptr()` 的差。預期 `price` 是 `0`、`qty` 是 `9`、`tax2` 是 `21`。
5. 你只寫了 `next`，試著直接使用這些功能：
   - `for token in Tokenizer::new(…)`
   - `.filter(…).count()` 算出有幾個 `Ident`（預期 `3`）
   - `.collect::<Vec<_>>()`

## 限制

- `next` 裡不准配置 heap（不准出現 `String`、`to_string`、`to_owned`、`format!`）
- 不准出現 `unsafe`

## 實驗

每個都先預測「編不編得過」，再試。

- A. 原始字串比 token 早消失：
  ```rust
  let tokens: Vec<Token>;
  {
      let s = String::from("a + b");
      tokens = Tokenizer::new(&s).collect();
  }
  println!("{:?}", tokens);
  ```
- B. `Tokenizer` 比 token 早消失，原始字串還在：
  ```rust
  let s = String::from("a + b");
  let first;
  {
      let mut t = Tokenizer::new(&s);
      first = t.next();
  }
  println!("{:?}", first);
  ```
- C. 把 `Ident(&'a str)` 改成 `Ident(String)`，把所有 `'a` 拿掉，讓程式編得過。比較前後兩個版本

## 在註解裡回答（先寫預測，再跑）

1. 實驗 A 和 B 一個編不過、一個編得過。根據這個結果，`'a` 描述的是「誰」必須活著？分別說明為什麼下面兩個說法是錯的：
   - 「`'a` 是 `Tokenizer` 活著的期間」
   - 「`'a` 是某個函式的 stack frame」
2. 一個 `Token::Ident` 在記憶體裡實際存了什麼？tokenize 整個字串並 `collect` 的過程中，heap 配置總共發生幾次、是誰配置的？
3. 實驗 C：改成 `String` 之後，lifetime 為什麼就不需要了？換來的代價是什麼？
4. 你只實作了 `next`，為什麼 `.filter()`、`.collect()`、`for` 都能用？那些方法的程式碼寫在哪一層？
5. `impl<'a> Iterator for Tokenizer<'a>` 這一行出現兩次 `'a`，哪一個是宣告、哪一個是使用？

## 交卷

commit 之後說「複盤 q09」。
