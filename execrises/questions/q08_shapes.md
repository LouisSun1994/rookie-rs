# q08：圖形與 trait

檔案：[`src/bin/q08_shapes.rs`](../src/bin/q08_shapes.rs)

## 背景

trait 定義「一組能力」，不同型別各自實作。用 trait 寫函式有兩條路：

- generics（泛型）：`fn f<T: Shape>(s: &T)`
- trait object：`fn f(s: &dyn Shape)`

兩者寫起來很像，底層完全不同。這題兩種都寫，然後比較。

## 要做的事

1. 定義 trait：
   ```rust
   trait Shape {
       fn name(&self) -> String;
       fn area(&self) -> f64;
       // 有預設實作的方法
       fn describe(&self) -> String {
           format!("{} 面積 {:.2}", self.name(), self.area())
       }
   }
   ```
2. 三個型別各自實作 `Shape`：
   - `Circle { r: f64 }`
   - `Rect { w: f64, h: f64 }`
   - `Triangle { a: f64, b: f64, c: f64 }`（三邊長，面積用海龍公式）

   其中一個要覆寫 `describe`，多印一項自己特有的資訊。
3. 幫 `Rect` 實作 `std::fmt::Display`，讓 `println!("{}", rect)` 印出 `Rect(3x4)`。
4. 三個印出 `describe()` 的函式：
   - `fn print_static<T: Shape>(s: &T)`
   - `fn print_impl(s: &impl Shape)`
   - `fn print_dyn(s: &dyn Shape)`
5. `fn sum_same<T: Shape>(items: &[T]) -> f64`：同一種圖形的面積總和。
6. 把三種圖形放進同一個 `Vec<Box<dyn Shape>>`，然後：
   - `fn total_area(shapes: &[Box<dyn Shape>]) -> f64`
   - `fn largest(shapes: &[Box<dyn Shape>]) -> Option<&dyn Shape>`
7. `main` 用 `Circle { r: 1.0 }`、`Rect { w: 3.0, h: 4.0 }`、`Triangle { a: 3.0, b: 4.0, c: 5.0 }` 驗證：

   | 項目 | 預期 |
   |---|---|
   | 圓面積 | `3.14` |
   | 矩形面積 | `12.00` |
   | 三角形面積 | `6.00` |
   | `total_area` | `21.14` |
   | `largest` | 矩形 |

## 實驗

每個都先預測，再試。

- A. 宣告 `let v: Vec<dyn Shape> = Vec::new();`（沒有 `Box`）
- B. 把一個 `Circle` 和一個 `Rect` 放進同一個陣列，傳給 `sum_same`
- C. 印出這四個型別的 `std::mem::size_of`，先寫下預測：
  `&Circle`、`&dyn Shape`、`Box<Circle>`、`Box<dyn Shape>`

## 在註解裡回答（先寫預測，再跑）

1. 實驗 C：`&dyn Shape` 比 `&Circle` 多出來的那些 bytes 裡面放的是什麼？
2. `print_static(&c)` 和 `print_dyn(&c)` 都會呼叫到 `Circle` 的 `area`。「要呼叫哪一個 `area`」這件事，在兩種寫法裡分別是誰決定的、在編譯期還是執行期決定的？
3. `print_static` 分別用三種圖形各呼叫一次。編出來的執行檔裡，`print_static` 的機器碼有幾份？`print_dyn` 有幾份？各自的代價是什麼？
4. 實驗 A：編譯器抱怨的是什麼？為什麼加上 `Box` 就可以？
5. `print_impl` 的底層和 `print_static`、`print_dyn` 哪一個相同？

## 交卷

commit 之後說「複盤 q08」。
