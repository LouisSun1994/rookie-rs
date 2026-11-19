# q03：玩家背包

檔案：[`src/bin/q03_inventory.rs`](../src/bin/q03_inventory.rs)

## 背景

用 `struct` 把資料綁在一起，用 `impl` 掛上方法。方法的第一個參數有三種：
`&self`（借來看）、`&mut self`（借來改）、`self`（整個拿走）。這題三種都會用到。

## 資料

```rust
struct Item {
    name: String,
    weight: u32,
    value: u32,
}

struct Player {
    name: String,
    hp: u32,
    capacity: u32, // 背包最多能裝的總重量
    bag: Vec<Item>,
}
```

## 要做的事

1. `Player::new(name: &str, capacity: u32) -> Player`：`hp` 設 100，背包是空的。
2. `fn total_weight(&self) -> u32`：背包目前的總重量。
3. `fn pick_up(&mut self, item: Item) -> bool`：放進去之後不超過 `capacity` 就放進背包並回傳 `true`，否則不放、回傳 `false`。
4. `fn most_valuable(&self) -> Option<&Item>`：回傳 `value` 最高的物品；背包是空的就回傳 `None`。
5. `fn drop_all(self) -> Vec<Item>`：玩家消失，把整個背包交出來。
6. `main` 建立 `capacity` 為 10 的玩家，依序撿下面四樣，印出每次的結果：

   | 物品 | weight | value | 預期 |
   |---|---|---|---|
   | sword | 4 | 50 | `true` |
   | shield | 5 | 30 | `true` |
   | anvil | 20 | 5 | `false` |
   | potion | 1 | 80 | `true` |

   接著印出：`total_weight()` 是 `10`、`most_valuable()` 是 `potion`、`drop_all()` 回傳的 `Vec` 長度是 `3`。

## 限制

- `total_weight` 和 `most_valuable` 用 `for` 加一個自己維護的變數來做，不要用 `.sum()`、`.max_by_key()` 等 iterator 方法
- 不准出現 `.clone()`
- 可以用 `#[derive(Debug)]` 方便印出

## 實驗

每個都先預測，再試。預測和結果直接寫在各實驗下面。

### 實驗 A

把 `let mut p = Player::new(…)` 的 `mut` 拿掉，再呼叫 `pick_up`

**預測**：fail,fn會改bag

**結果**：

### 實驗 B

在 `drop_all()` 之後呼叫 `p.total_weight()`

**預測**：fail,p已經被move(drop_all)且fn完後釋放

**結果**：

### 實驗 C

印出 `std::mem::size_of::<Item>()` 和 `std::mem::size_of::<Player>()`。先算出你預測的數字

**預測**：這邊根據rustc優化後計算方式，首先Item.name我用enum寫法，我會給兩版打小，分別是enum,&str，前者4+4+1，但因為rustc 4bytes一組所以是12bytes;後者，16+4+4=24bytes。

再者Player,Vec->24bytes,String->24bytes,24+24+4+4=56。

**結果**：

### 實驗 D

幫 `Item` 實作 `Drop`，內容只做一件事：印出 `drop: {name}`。觀察 anvil 那一行出現在輸出的哪個位置，以及程式結束前其他物品的順序

**預測**：沒打印(anvil)，裝不進去包包。

**結果**：

## 問題

回答直接寫在各題下面。

### 問題 1

實驗 C 你預測幾 bytes、實際幾 bytes？背包多放十個物品，`size_of::<Player>()` 會變嗎？為什麼？

**回答**： 
```
不會變，因為Vec的elements是在heap
```

### 問題 2

撿完 sword 和 shield 的那一刻，畫出 stack 和 heap 的配置圖。heap 上總共有幾塊？每一塊的擁有者是誰？

**回答**：
```
針對player.bag回答，stack會有Vec狀態，分別指向heap兩個位置表示sword,shield。
```

### 問題 3

anvil 被拒絕之後，它的 `name` 佔的那塊 heap 是在哪一行程式碼的位置被釋放的？釋放的程式碼是誰寫的、誰放進去的、哪條 thread 執行的？

**回答**：
```
這題我用enum版本回答，anvil item在 pick_up後就回收了，但是name不是會不見，這是寫在.rodata的靜態內容。
```

### 問題 4

`most_valuable` 回傳 `Option<&Item>`。如果改成回傳 `Option<Item>`，函式內容必須怎麼改？呼叫完之後背包會變成什麼樣？

**回答**：

```=rust
    fn most_valuable(&self) -> Option<Item> {
        let mut max_value_item: Option<Item> = None;
        for item in &self.bag {
            if max_value_item.is_none() || item.value > max_value_item.unwrap().value {
                max_value_item = Some(*item);
            }
        }
        max_value_item
    }
```

```
呼叫完之後，本來的包背這個item就被回收了，除非改成clone()
```

### 問題 5

`drop_all(self)` 的 `self` 不是 `&self`。呼叫的那一刻，`p` 的哪些部分被搬走、哪些被釋放？

**回答**：

```
move的話，fn結束後回收instance;
反之，borrow的話還在。
```

## 交卷

commit 之後說「複盤 q03」。
