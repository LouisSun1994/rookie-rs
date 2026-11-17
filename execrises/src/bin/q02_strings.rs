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
