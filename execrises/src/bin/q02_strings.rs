// ===== q02 觀察與回答 =====
//
// 【實驗 A】字串交給 shout 之後，還能不能用
//     let loud = shout(name);
//     println!("{name}");
//   預測（編得過 / 編不過，為什麼）： 不行，因為已經被move
//   跑完後（錯誤訊息的重點）：
//
// 【實驗 B】借出去看的同時，拿去改
//     let w = first_word(&name);
//     append_bang(&mut name);
//     println!("{w}");
//   預測：hello ferris!
//   跑完後： error[E0502]: cannot borrow `name` as mutable because it is also borrowed as immutable
//
// 【實驗 C】和 B 一樣，但修改之後不再使用 w（只留 B 的前兩行）
//     let w = first_word(&name);
//     append_bang(&mut name);
//   預測：
//   跑完後：看到這題，我猜測，可以compile是因為，w沒有被引用了，所以first_word borrow 釋放，因此append_bang PASS。
//
// 【實驗 D】.clone() 到底做了什麼
//     let copy = name.clone();
//   預測：copy 跟 name 是不同位址
//   name.as_ptr() ：
//   copy.as_ptr() ：
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
// 【問題 1】上面的位址哪些相同、哪些不同？
//           &str、&mut String、String 三種傳法，各自讓 heap 上的字元被複製了幾次？
//   預測：&str,&mut String 相同;String不同(Copy new addr)
//   跑完後：
//
// 【問題 2】實驗 B 假設編譯器放行，執行期可能出什麼事？
//           （提示：push 在容量不夠時會做什麼、w 指向哪裡）
//   回答： 假設放行，w會亂掉。
//
//     first_word(&name);
//     append_bang(&mut name);
//     println!("{name}");
//
// 【問題 3】實驗 B 和 C 只差一行 println!。兩者的結果相同嗎？
//           如果不同，編譯器判斷的依據是什麼？
//   回答：我已經跑過，不一樣，w沒有被引用所以釋放，因此append可以borrow。
//
// 【問題 4】first_word("no_space") 的字元住在哪裡（stack / heap / 其他）？
//           誰負責釋放、什麼時候？
//   回答："no_space" 這會在stack，main stack結束才釋放。
//
// 【問題 5】&name 的型別是 &String，為什麼能傳給要 &str 的函式？
//   回答：這也是我的好奇，我只能猜測compile有做手腳。

fn main() {
    let mut name = String::from("hello ferris");
    println!("{:p}", name.as_ptr());
    count_vowels(&name);
    println!("count_vowels -> {:p}", name.as_ptr());
    let fst = first_word(&name);
    println!("first_word -> {:p}, {:p}", name.as_ptr(), fst.as_ptr());
    append_bang(&mut name);
    println!("append_bang -> {:p}", name.as_ptr());
    let loud = shout(name);
    println!("shout -> {:p}", loud.as_ptr());
    first_word("no_space");
    println!("first_word -> {:p}", loud.as_ptr());
    count_vowels("AEIOU xyz");
    println!("count_vowels -> {:p}", loud.as_ptr());
}

// 回傳母音（a e i o u，不分大小寫）的個數。
fn count_vowels(s: &str) -> usize {
    s.to_lowercase().matches(['a', 'e', 'i', 'o', 'u']).count()
}

// 回傳第一個空白之前的部分；沒有空白就回傳整個字串。
fn first_word(s: &str) -> &str {
    match s.split_whitespace().next() {
        Some(w) => w,
        None => s,
    }
}

// 在尾端加一個 !。
fn append_bang(s: &mut String) {
    s.push_str("!");
    println!("append_bang(inside) -> {:p}", s.as_ptr());
}

// 轉成大寫、尾端再加一個 !，回傳。
fn shout(s: String) -> String {
    println!("shout(inside) -> {:p}", s.as_ptr());
    s.to_ascii_uppercase().push_str("!");
    s
}
