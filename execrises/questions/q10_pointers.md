# q10：鏈結串列與樹

檔案：[`src/bin/q10_pointers.rs`](../src/bin/q10_pointers.rs)

## 背景

到目前為止每個值都只有一個擁有者，而且大小在編譯期就知道。這題處理兩個例外：

- 型別裡包含自己（鏈結串列的節點指向下一個節點）→ `Box<T>`
- 一個值需要多個擁有者（樹的子節點要能指回父節點）→ `Rc<T>`、`Weak<T>`、`RefCell<T>`

## 第一部分：用 `Box` 做 stack

```rust
struct Node {
    val: i32,
    next: Option<Box<Node>>,
}

struct Stack {
    head: Option<Box<Node>>,
    len: usize,
}
```

1. 實作 `Stack::new()`、`push(&mut self, val: i32)`、`pop(&mut self) -> Option<i32>`、`peek(&self) -> Option<&i32>`。
2. `main` 驗證：push 1、2、3 → `peek` 是 `Some(3)` → 連續 `pop` 得到 3、2、1、`None`。

提示：`Option::take()` 會把值拿走並在原地留下 `None`。

## 第二部分：用 `Rc` 做樹

```rust
struct TreeNode {
    name: String,
    parent: RefCell<Weak<TreeNode>>,
    children: RefCell<Vec<Rc<TreeNode>>>,
}
```

3. `fn new_node(name: &str) -> Rc<TreeNode>`
4. `fn add_child(parent: &Rc<TreeNode>, child: Rc<TreeNode>)`：把 child 加進 parent 的 `children`，並把 child 的 `parent` 指回去。
5. `fn path(node: &Rc<TreeNode>) -> String`：從這個節點往上走到根，回傳像 `root/branch/leaf` 的路徑。
6. `main` 建一棵 `root → branch → leaf` 的樹，在每個步驟之後印出三個節點的 `Rc::strong_count` 和 `Rc::weak_count`，最後印出 `path(&leaf)`。
7. 幫 `TreeNode` 實作 `Drop`，只印 `drop: {name}`，觀察程式結束時的順序。

## 限制

- 不准出現 `unsafe`
- 第一部分不准用 `Vec`、`Rc`

## 實驗

每個都先預測，再試。

- A. 把 `next: Option<Box<Node>>` 的 `Box` 拿掉，變成 `next: Option<Node>`
- B. 用迴圈 push 一百萬個節點，然後讓 `Stack` 離開 scope。debug 和 `--release` 都跑一次。
  看到結果之後，幫 `Stack` 實作 `Drop` 把問題修掉
- C. 把 `parent` 的型別改成 `RefCell<Option<Rc<TreeNode>>>`（用 `Rc` 而不是 `Weak`），其他照舊。觀察 `drop:` 那幾行
- D. 對同一個 `RefCell` 連續呼叫兩次 `borrow_mut()`，兩個結果都留著不放

## 在註解裡回答（先寫預測，再跑）

1. 實驗 A：編譯器抱怨什麼？加了 `Box` 之後 `size_of::<Node>()` 是多少？為什麼這樣就算得出來？
2. 實驗 B：一百萬個節點是誰在釋放、用什麼方式一個接一個釋放的？這和 q01 遇到的是不是同一種問題？你的 `Drop` 改變了什麼？
3. `Rc` 的計數器存在哪裡？`Rc::clone(&x)` 複製了什麼、沒複製什麼？計數是誰在什麼時候減一的？
4. 實驗 C：為什麼沒有任何一個節點被釋放？程式結束之後，那些記憶體最後怎麼了、是誰處理的？
5. 實驗 D：這個錯誤出現在編譯期還是執行期？`RefCell` 把借用規則的檢查從哪個時期搬到了哪個時期，換來什麼、付出什麼？
6. `Rc` 能不能跨 thread 使用？先猜，q11 會驗證。

## 交卷

commit 之後說「複盤 q10」。
