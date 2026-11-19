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

**複盤**：預測正確。

```
error[E0596]: cannot borrow `p` as mutable, as it is not declared as mutable
```

`pick_up` 的簽名是 `&mut self`，`p.pick_up(…)` 等於 `Player::pick_up(&mut p, …)`。要對 `p` 取 `&mut`，`p` 就必須宣告成 `let mut`。編譯期檢查。

### 實驗 B

在 `drop_all()` 之後呼叫 `p.total_weight()`

**預測**：fail,p已經被move(drop_all)且fn完後釋放

**結果**：

**複盤**：預測正確。

```
error[E0382]: borrow of moved value: `p`
```

修一個地方：「fn 完後釋放」不是整個 `p` 都被釋放。`bag` 被交回給呼叫端，只有 `name` 被釋放，細節見問題 5。

### 實驗 C

印出 `std::mem::size_of::<Item>()` 和 `std::mem::size_of::<Player>()`。先算出你預測的數字

**預測**：這邊根據rustc優化後計算方式，首先Item.name我用enum寫法，我會給兩版打小，分別是enum,&str，前者4+4+1，但因為rustc 4bytes一組所以是12bytes;後者，16+4+4=24bytes。

再者Player,Vec->24bytes,String->24bytes,24+24+4+4=56。

**結果**：

**複盤**：三個數字都算對了，實測 enum 版 `Item` 是 12、`Player` 是 56。

修一個詞：「rustc 4 bytes 一組」的正式名稱是 alignment（對齊）。規則是兩條：

1. struct 的對齊 = 欄位中最大的對齊
2. struct 的大小必須是對齊的整數倍，不足的補空白（padding）

| `name` 的型別 | 欄位加總 | 對齊 | `size_of::<Item>()` |
|---|---|---|---|
| `ItemName`（enum，1 byte） | 1 + 4 + 4 = 9 | 4（來自 `u32`） | 12 |
| `&str` | 16 + 4 + 4 = 24 | 8（來自指標） | 24 |
| `String`（題目原本的設計） | 24 + 4 + 4 = 32 | 8 | 32 |

`Player`：24 + 24 + 4 + 4 = 56，對齊 8，剛好整除。

### 實驗 D

幫 `Item` 實作 `Drop`，內容只做一件事：印出 `drop: {name}`。觀察 anvil 那一行出現在輸出的哪個位置，以及程式結束前其他物品的順序

**預測**：沒打印(anvil)，裝不進去包包。

**結果**：

**複盤**：第三輪已改成 std 的 `Drop`，實驗完成。預測是錯的，下面保留前兩輪的問題和正確的觀察。

第一版寫的是自己定義的 `trait Drop { fn drop(&self); }`，第二版改成 `impl Item { fn drop(&self) }`。兩者都只是「一個名字叫 drop 的普通方法」，編譯器不會自動呼叫，所以才需要在 `main` 手動寫 `p.bag.iter().for_each(|i| i.drop())`。那是你在某個時間點主動印出三行字，和「值被釋放」無關。

真正的 `Drop` 是 std 提供的 trait，不需要自己宣告，直接替 `Item` 實作。`name` 維持 enum 也可以，`Drop` 和型別裡有沒有 heap 無關：

```rust
impl Drop for Item {
    fn drop(&mut self) {              // 簽名固定是 &mut self
        println!("drop: {}", self.name.to_str());
    }
}
```

它的特別之處：**呼叫它的是 rustc 插入的程式碼，不是你。** 值的擁有者消失的那一刻自動執行。手動寫 `item.drop()` 反而會被拒絕（`error[E0040]: explicit destructor calls not allowed`）。

最終版本的輸出：

```
pickup sword result is true
pickup shield result is true
  drop: anvil                    ← 被拒絕的那一刻就印了，比下一行還早
pickup anvil result is false
pickup potion result is true
total weight is 10
drop all is 3
  drop: sword                    ← drop_all() 回傳的 Vec 沒有人接，
  drop: shield                      那一行敘述結束時就被釋放
  drop: potion
```

預測「anvil 沒印，因為裝不進包包」剛好相反：**正因為裝不進包包，它沒有任何擁有者，所以最先被釋放。** 裝進去的三樣有 `bag` 擁有，活到 `bag` 消失為止。

## 問題

回答直接寫在各題下面。

### 問題 1

實驗 C 你預測幾 bytes、實際幾 bytes？背包多放十個物品，`size_of::<Player>()` 會變嗎？為什麼？

**回答**： 
```
不會變，因為Vec的elements是在heap
```

**複盤**：正確。`size_of` 是編譯期就固定的數字，只算 stack 上那一份（`Vec` 的 ptr、len、cap 三個欄位）。元素在 heap，放再多也不影響。

### 問題 2

撿完 sword 和 shield 的那一刻，畫出 stack 和 heap 的配置圖。heap 上總共有幾塊？每一塊的擁有者是誰？

**回答**：
```
針對player.bag回答，stack會有Vec狀態，分別指向heap兩個位置表示sword,shield。
```

**複盤**：模型要修一處：`Vec` 不是「分別指向 heap 兩個位置」。

> `Vec` 只有**一個**指標，指向**一塊**連續的 heap 空間，元素在裡面一個挨一個排好。

實測：`bag[0]` 和 `bag[1]` 的位址相差 12 bytes，剛好是 `size_of::<Item>()`；`bag.capacity()` 是 4（第一次 `push` 就配置了 4 個位置）。

你的版本（`name` 是 enum）在那一刻的配置圖，heap 上共 3 塊：

```
STACK（main 的 frame）                        HEAP
list: [ptr | len 4 | cap 4] ──────────►  [sword][shield][anvil][potion]   一塊，4 × 12 bytes
p:
  name:     [ptr | len 6 | cap 6] ────►  "player"                          一塊，6 bytes
  hp:       100
  capacity: 10
  bag:      [ptr | len 2 | cap 4] ────►  [sword][shield][ 空 ][ 空 ]        一塊，4 × 12 bytes
```

| heap 區塊 | 擁有者 |
|---|---|
| `list` 的元素 | `list` |
| `"player"` | `p.name` |
| `bag` 的元素 | `p.bag` |

題目原本的設計（`name: String`）會多出兩塊：sword 和 shield 各自的名字。擁有關係變成一棵樹：`p` → `bag` → 那塊連續空間裡的每個 `Item` → 各自的 `name`。

### 問題 3

anvil 被拒絕之後，它的 `name` 佔的那塊 heap 是在哪一行程式碼的位置被釋放的？釋放的程式碼是誰寫的、誰放進去的、哪條 thread 執行的？

**回答**：
```
這題我用enum版本回答，anvil item在 pick_up後就回收了，但是name不是會不見，這是寫在.rodata的靜態內容。
```

**複盤**：以你的 enum 版本來說，回答正確：

- `Item` 裡沒有任何東西在 heap，所以沒有 heap 要釋放
- `item` 那 12 bytes 在 `pick_up` 的 frame 裡，`return false` 時跟著 frame 一起消失
- `"anvil"` 這些字串在執行檔的唯讀資料區，不會被釋放

但也因為改成 enum，這題原本要問的事情被繞過了。`name: String` 的版本答案是：

| 問 | 答 |
|---|---|
| 在哪裡釋放 | `pick_up` 的 `return false` 那個出口 |
| 釋放的程式碼是誰寫的 | std：`String` 的 `Drop` 實作，內容是呼叫 allocator 把那塊還回去 |
| 誰放進去的 | rustc：編譯期在「`item` 還有擁有權、但函式要返回」的出口插入呼叫 |
| 哪條 thread 執行 | 呼叫 `pick_up` 的那條，也就是 main thread，user mode |

另一條路徑（`self.bag.push(item)`）不會插入釋放：所有權已經搬進 `bag`，`item` 這個名字不再擁有任何東西。實驗 D 的輸出裡 `drop: anvil` 出現在 `pickup anvil result is false` 之前，就是這件事的證據。

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

**複盤**：這段程式編不過，有兩個錯誤：

```
error[E0507]: cannot move out of `*item` which is behind a shared reference
error[E0382]: use of moved value: `max_value_item`
```

1. `Some(*item)`：`item` 是 `&Item`。透過唯讀借用只能看，不能把東西搬走。
2. `max_value_item.unwrap()`：`unwrap` 會把 `Option` 裡的值拿走。原本的 `Option<&Item>` 是 `Copy` 所以沒事，改成 `Option<Item>` 之後第一次 `unwrap` 就把它搬空了。

要回傳 `Option<Item>`，只有兩條路：

| 做法 | 簽名 | 呼叫後的背包 |
|---|---|---|
| 複製一份交出去 | `&self`，內容用 `clone()` | 不變，世界上多了一份 |
| 把那一件從背包拿出來 | 必須改成 `&mut self`，找到位置後 `self.bag.remove(i)` | 少一件 |

修一個詞：第二種做法裡那件物品不是「被回收」，是所有權從 `bag` 搬到了呼叫端手上，它還活著。

### 問題 5

`drop_all(self)` 的 `self` 不是 `&self`。呼叫的那一刻，`p` 的哪些部分被搬走、哪些被釋放？

**回答**：

```
move的話，fn結束後回收instance;
反之，borrow的話還在。
```

**複盤**：方向對，但題目問的是「哪些部分」，要拆開講：

| `p` 的欄位 | 呼叫 `drop_all` 之後 |
|---|---|
| `bag` | **搬走**：ptr、len、cap 三個欄位回傳給呼叫端，heap 上那塊元素原地不動 |
| `name` | **釋放**：`drop_all` 結束時沒人接手，`String` 的 `Drop` 把 `"player"` 那塊還給 allocator |
| `hp`、`capacity` | 純數字，沒有東西要釋放，隨 frame 消失 |

`self.bag` 這個寫法叫 partial move（部分搬移）：只搬走一個欄位，其餘欄位在函式結束時各自處理。

## 程式碼複盤

第三輪，通過。

✅ 三輪下來解決的

- `Drop`：從自訂 trait、普通方法，改成實作 std 的 `Drop`，`main` 裡手動呼叫的那一行也拿掉了。
- `.clone()` 拿掉，改成 `for item in list` 把所有權逐一交出來。
- `name` 用 enum：保留。第一輪說「改成 enum 後實驗 D 沒東西可看」是錯的，`Drop` 可以實作在任何型別上，和有沒有 heap 無關。enum 版唯一少掉的是「`name` 的 heap 何時被釋放」這個觀察，問題 3 的複盤已經用文字補上。

🟡 沒改的建議（不影響結案）

- `most_valuable` 印的是 `80`，題目預期是物品名稱 `potion`：把 `f.value.to_string()` 換成 `f.name.to_str()`。
- `most_valuable` 裡的 `is_none() || … .unwrap()` 用 `match` 表達更直接，也不需要 `unwrap`：
  ```rust
  match best {
      Some(b) if b.value >= item.value => {}
      _ => best = Some(item),
  }
  ```
- `pick_up` 的 `self.capacity - self.total_weight()` 在 `u32` 上做減法，哪天總重超過容量就會溢位。改成加法比較：`self.total_weight() + item.weight > self.capacity`。
- `map_or("null".to_string(), …)`：Rust 沒有 null，`None` 的情況用 `match` 印一句「背包是空的」。
- `#[derive(Clone)]` 已經沒有人用，可以拿掉。
- 四個實驗的「結果」沒有填，正確的結果都寫在各自的複盤裡。

🟢 做得好

- 三種 `self` 都用對：`total_weight(&self)`、`pick_up(&mut self)`、`drop_all(self)`。
- `most_valuable` 回傳的是 reference，沒有複製任何物品。
- `println!("…", item.name.to_str(), p.pick_up(item))`：先拿名字、再把 `item` 交出去，同一行就解決了「交出去之後不能再用」的問題。能這樣寫是因為 `to_str` 回傳 `&'static str`，拿到的字串不依賴 `item`。
- 實驗 C 把 padding 算進去了。

## 交卷

commit 之後說「複盤 q03」。
