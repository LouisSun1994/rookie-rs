# q12：echo 與聊天室 server

檔案：[`src/bin/q12_chat.rs`](../src/bin/q12_chat.rs)

接在 [notes/02](../../notes/02-tokio-runtime-os-scheduling.md) 之後：那份筆記裡推過的模型，這題用程式實際觀察。

## 準備

這題第一次用到外部 crate：

```bash
cargo add tokio --features full
```

測試用兩個以上的終端機各自執行 `nc 127.0.0.1 8080`，打字後按 Enter 送出一行。

## 第一階段：echo server

1. `#[tokio::main]`，用 `tokio::net::TcpListener` 監聽 `127.0.0.1:8080`。
2. 迴圈 `accept`，每條連線 `tokio::spawn` 一個 task。
3. task 裡逐行讀取，把同一行寫回去，連線關閉就結束。
4. 每處理一行，在 server 端印出 `std::thread::current().id()`、對方位址、內容。

## 第二階段：聊天室

5. 用 `tokio::sync::broadcast` channel：任何人送出一行，其他所有連線都收到 `<位址>: <內容>`，自己不會收到自己的。
6. 每個 task 用 `tokio::select!` 同時等兩件事：socket 來了一行，或 broadcast 來了一則訊息。
7. 有人連線和離線時，廣播 `<位址> 加入` / `<位址> 離開`。

## 限制

- 不准出現 `unsafe`
- async 函式裡不准出現 `std::thread::sleep` 和 `std::sync::Mutex`（實驗除外）

## 實驗

每個都先預測，再試。預測和結果直接寫在各實驗下面。

### 實驗 A

連續送很多行，觀察同一條連線的 thread id 會不會變

**預測**：

**結果**：

### 實驗 B

改成 `#[tokio::main(flavor = "current_thread")]`，在處理每一行的地方加 `std::thread::sleep(Duration::from_secs(5))`。
開兩個 client，A 送一行後 B 立刻送一行，觀察 B 多久後收到回應。
然後把它換成 `tokio::time::sleep(…).await` 再做一次

**預測**：

**結果**：

### 實驗 C

回到預設的 multi-thread runtime，保留 `std::thread::sleep`，重做 B 的觀察

**預測**：

**結果**：

### 實驗 D

在 task 裡拿一個 `std::sync::Mutex` 的 guard，不釋放，接著 `.await` 任何東西

**預測**：

**結果**：

### 實驗 E

開十條連線，都不送資料。用 `ps -M <pid>`（macOS）看這個 process 有幾條 OS thread

**預測**：

**結果**：

## 問題

回答直接寫在各題下面。

### 問題 1

實驗 A：同一條連線的 task 為什麼可以在不同 thread 上執行？task 在兩次執行之間，「它的狀態」存在哪裡？

**回答**：

### 問題 2

實驗 B、C：三種組合（`current_thread` + `std::thread::sleep`、`current_thread` + `tokio::time::sleep`、multi-thread + `std::thread::sleep`）的結果各是什麼？分別說明那 5 秒內，哪條 thread 在做什麼、另一個 client 的 task 在哪裡。

**回答**：

### 問題 3

實驗 D：編譯器抱怨什麼？它和 q11 的實驗 A 是不是同一條規則？

**回答**：

### 問題 4

實驗 E：十條連線對應幾個 task、幾條 OS thread？一條閒置的連線，它的 task 此刻在不在任何佇列裡？誰會在資料到達時把它放回去？

**回答**：

### 問題 5

寫出完整時間軸：「client A 按下 Enter」到「client B 的終端機顯示那一行」。
每一行標明：哪條 thread、user mode 還是 kernel mode、執行誰的程式碼、發生什麼事。

**回答**：

## 交卷

commit 之後說「複盤 q12」。
