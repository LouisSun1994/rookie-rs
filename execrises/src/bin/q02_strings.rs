// ===== q02 觀察與回答 =====
//
// 【實驗 A】字串交給 shout 之後，還能不能用
//     let loud = shout(name);
//     println!("{name}");
//   預測（編得過 / 編不過，為什麼）： 不行，因為已經被move
//   跑完後（錯誤訊息的重點）：
//
//   [複盤] 預測正確。
//          error[E0382]: borrow of moved value: `name`
//          shout(name) 把所有權交給了 shout 的參數 s，main 的 name 從那一行起不能再用。
//          這是編譯期檢查；執行期 name 那三個欄位其實還留在 main 的 stack 上，
//          只是編譯器不准你再碰它。
//
// 【實驗 B】借出去看的同時，拿去改
//     let w = first_word(&name);
//     append_bang(&mut name);
//     println!("{w}");
//   預測：hello ferris!
//   跑完後： error[E0502]: cannot borrow `name` as mutable because it is also borrowed as immutable
//
//   [複盤] 預測錯在兩處：
//          1. 編不過。w 還要在第三行被使用，所以對 name 的唯讀借用一直持續到第三行；
//             第二行想要 &mut（獨佔）借用，兩者重疊，編譯器拒絕。
//          2. 就算放行，w 也不會是 "hello ferris!"。
//             w 是（起點位址, 長度 5），它不會跟著 name 變長，內容只會是 "hello"。
//
// 【實驗 C】和 B 一樣，但修改之後不再使用 w（只留 B 的前兩行）
//     let w = first_word(&name);
//     append_bang(&mut name);
//   預測：
//   跑完後：看到這題，我猜測，可以compile是因為，w沒有被引用了，所以first_word borrow 釋放，因此append_bang PASS。
//
//   [複盤] 正確。修一個詞：「釋放」→「借用結束」。
//          借用持續到「最後一次使用」為止，不是到變數離開 scope 為止。
//          w 在第一行之後沒有再被用到，借用在第一行就結束，第二行的 &mut 不衝突。
//          借用是純編譯期的概念，執行期沒有任何東西被釋放。
//
// 【實驗 D】.clone() 到底做了什麼
//     let copy = name.clone();
//   預測：copy 跟 name 是不同位址
//   name.as_ptr() ：
//   copy.as_ptr() ：
//
//   [複盤] 預測正確，兩個位址不同。
//          clone 向 allocator 要了一塊新的 heap 空間，把所有字元複製過去。
//          這是「字元真的被複製」的操作，和傳參數不同（完整清單見問題 1）。
//
// 【位址觀察】第 6 步印出來的位址
//   main 呼叫 count_vowels 前 ： count_vowels 前後 位址一樣 (Heap)
//   count_vowels 裡           ：
//   first_word 裡             ： first_word 竟然一樣，明明取空白前的那一串 (Heap)
//   append_bang 裡            ： append_bang 一樣 (Heap) (提問:為什麼append_bang會變位址?)
//   main 呼叫 shout 前        ： invoke時候就已經Copy新位址
//   shout 裡                  ： 使用invoke的位址作調整，回傳(同上)
//   loud（shout 回傳後）      ： (同上)
//
//   [複盤] 用 X、Y、Z 代表三個不同的位址，完整時間軸：
//
//          t0  String::from("hello ferris")   配置 12 bytes，從唯讀資料區
//                                             複製字元進去，len=12 cap=12      位址 X
//          t1  count_vowels(&name)            借用，只傳位址                   位址 X
//          t2  first_word(&name)              借用；回傳的切片起點也是 X，長度 5
//          t3  append_bang → push             需要 13 bytes，cap 不夠
//                                             → std 向 allocator 要更大的一塊
//                                             → 原地能擴大就留在 X；
//                                               不能就搬到新位置、釋放舊的     位址 Y（或仍是 X）
//          t4  shout(name)                    move：只搬 stack 上三個欄位      位址 Y
//          t5  shout 裡的 s                   和 t4 相同                       位址 Y
//          t6  s.to_ascii_uppercase()         配置一塊全新的，寫入大寫字元     位址 Z
//          t7  shout 結束                     rs 交給呼叫端；s 沒人要了，
//                                             Y 那塊在這裡被釋放
//          t8  loud                           就是 rs                          位址 Z
//
//          逐格訂正：
//          - first_word「竟然一樣」：&str 是（起點位址, 長度）。"hello" 的起點
//            就是整個字串的起點，所以位址相同，差別只在長度是 5。
//          - 「為什麼 append_bang 會變位址」：t3。和 &mut 無關，是 push 遇到容量不夠。
//            會不會真的搬由 allocator 決定（macOS 上搬了，Linux 上同一份程式原地擴大）。
//          - 「invoke 時就已經 Copy 新位址」：錯。t4 的 move 沒有改變位址，
//            證據是「append_bang ->」和「shout(inside) ->」兩行印出的位址相同。
//          - loud 的位址和 shout 裡的不同，原因是 t6 的 to_ascii_uppercase
//            產生了新字串，不是 return 造成的。
//
// 【問題 1】上面的位址哪些相同、哪些不同？
//           &str、&mut String、String 三種傳法，各自讓 heap 上的字元被複製了幾次？
//   預測：&str,&mut String 相同;String不同(Copy new addr)
//   跑完後：
//
//   [複盤] 三種傳法，heap 上的字元被複製的次數都是 0。
//
//            &str         傳（位址, 長度），16 bytes
//            &mut String  傳一個位址，8 bytes，指向 main stack 上的 name
//            String       傳 stack 上的三個欄位（ptr, len, cap），24 bytes；
//                         heap 那塊原地不動，只是擁有者從 main 換成 shout
//
//          這份程式裡字元真的被複製或搬動的有四處，都不是「傳參數」造成的：
//            String::from（t0，從執行檔的唯讀資料區複製 12 bytes 到 heap）、
//            push 容量不夠（t3）、to_ascii_uppercase（t6）、clone（實驗 D）
//
// 【問題 2】實驗 B 假設編譯器放行，執行期可能出什麼事？
//           （提示：push 在容量不夠時會做什麼、w 指向哪裡）
//   回答： 假設放行，w會亂掉。
//
//     first_word(&name);
//     append_bang(&mut name);
//     println!("{name}");
//
//   [複盤] 方向對，精確的說法：
//          t3 的 push 可能把內容搬到 Y 並「釋放」X 那塊。
//          但 w 仍然是（X, 5），指著一塊已經還給 allocator 的記憶體。
//          這叫 dangling pointer（懸空指標）。之後讀 w 可能讀到垃圾、
//          讀到別人後來放進去的資料，或直接 crash，而且每次執行結果可能不同。
//          編譯器無法預知這次 push 會不會搬，所以只要 w 還會被用到，就一律不准改 name。
//
// 【問題 3】實驗 B 和 C 只差一行 println!。兩者的結果相同嗎？
//           如果不同，編譯器判斷的依據是什麼？
//   回答：我已經跑過，不一樣，w沒有被引用所以釋放，因此append可以borrow。
//
//   [複盤] 正確。依據是「w 之後還有沒有被使用」：
//            B：w 在 append_bang 之後還要用 → 唯讀借用跨過了 &mut → 衝突
//            C：w 在 append_bang 之前就用完了 → 兩個借用不重疊 → 通過
//          編譯器看的是每個借用「實際被使用的範圍」，不是變數的 scope。
//
// 【問題 4】first_word("no_space") 的字元住在哪裡（stack / heap / 其他）？
//           誰負責釋放、什麼時候？
//   回答："no_space" 這會在stack，main stack結束才釋放。
//
//   [複盤] 錯。字串常值的字元不在 stack 也不在 heap，是寫在「執行檔的唯讀資料區」。
//
//            "no_space" 的 8 個字元   執行檔的唯讀資料區（編譯時 rustc 就寫進去了）
//            指向它的（位址, 8）      stack，16 bytes
//
//          程式啟動時 kernel 把執行檔載入記憶體，那些字元就已經在了，
//          一直存在到 process 結束，沒有任何人釋放它。
//          這就是它的型別 &'static str 裡 'static 的意思。
//          驗證：印出 "no_space".as_ptr()、loud.as_ptr()、&loud，
//                會是三個落在不同區段的位址（唯讀資料區 / heap / stack）。
//
// 【問題 5】&name 的型別是 &String，為什麼能傳給要 &str 的函式？
//   回答：這也是我的好奇，我只能猜測compile有做手腳。
//
//   [複盤] 猜對了，機制叫 deref coercion（自動解參考轉換）：
//            std   ：String 實作了 Deref<Target = str>，宣告「我可以被當成 str 看」
//            rustc ：編譯時發現手上是 &String、需要的是 &str，自動插入轉換
//            執行期：只是從 String 的三個欄位裡取出 ptr 和 len，沒有複製任何字元
//          反過來不行：&str 不能自動變成 &String。所以唯讀參數寫 &str，
//          String 和字串常值兩種都能傳進來。

fn main() {
    let mut name = String::from("hello ferris");
    println!("{:p}", name.as_ptr());
    println!("{}, {:p}", count_vowels(&name), name.as_ptr());
    println!("{}, {:p}", first_word(&name), name.as_ptr());
    append_bang(&mut name);
    let loud = shout(name);
    println!("{}, {:p}", loud, loud.as_ptr());
    println!("{}", first_word("no_space"));
    println!("{}", count_vowels("AEIOU xyz"));
}

// 回傳母音（a e i o u，不分大小寫）的個數。
fn count_vowels(s: &str) -> usize {
    println!("  count_vowels(inside) -> {:p}", s.as_ptr());
    s.chars().filter(|f| "aeiouAEIOU".contains(*f)).count()
}

// 回傳第一個空白之前的部分；沒有空白就回傳整個字串。
fn first_word(s: &str) -> &str {
    println!("  first_word(inside) -> {:p}", s.as_ptr());
    match s.split_whitespace().next() {
        Some(w) => w,
        None => s,
    }
}

// 在尾端加一個 !。
fn append_bang(s: &mut String) {
    s.push_str("!");
    println!("  append_bang(inside) -> {:p}", s.as_ptr());
}

// 轉成大寫、尾端再加一個 !，回傳。
fn shout(s: String) -> String {
    println!("  shout(inside) -> {:p}", s.as_ptr());
    let mut rs = s.to_ascii_uppercase();
    rs.push_str("!");
    rs
}
