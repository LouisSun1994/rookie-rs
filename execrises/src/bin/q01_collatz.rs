// ===== q01 觀察與回答 =====
//
// 【實驗】把參數型別改成 u32，各跑一次
//   debug   ：（你看到什麼)
//    Compiling execrises v0.1.0 (/Users/admin/rs/rookie-rs/execrises)
//     Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
//      Running `target/debug/q01_collatz 3`
// recursive 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
// loop 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
//   release ：（你看到什麼)
//     Finished `release` profile [optimized] target(s) in 0.04s
//      Running `target/release/q01_collatz 3`
// recursive 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
// loop 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
//
// 【問題 1】兩種 build 行為為什麼不同？
//           那個「檢查」是誰放進去的（CPU / kernel / std / rustc）？什麼時間點放的？
//   預測：rustc compile 檢查
//   跑完後：
//
// 【問題 2】把記錄最佳值的變數的 mut 拿掉會怎樣？錯誤出現在編譯期還是執行期？
//   預測：compile fail
//   跑完後：

use std::env;
fn main() {
    let num = match env::args().nth(1) {
        Some(arg) => {
            let n = arg.parse::<u64>().expect("请输入正整数");
            if n < 1 || n > i32::MAX as u64 {
                panic!("请输入正整数(>0且<={})", i32::MAX);
            }
            n
        }
        None => 1,
    };

    // recursive
    let mut step = 0;
    let mut rs = String::new();
    let mut n = num;
    collatz_recursive(&mut n, &mut step, &mut rs);
    println!("recursive {}", rs);

    println!("loop {}", collatz(num));
}

fn collatz(mut num: u64) -> String {
    let mut step = 0;
    let mut rs = num.to_string();

    while num != 1 {
        num = if num % 2 == 0 { num / 2 } else { num * 3 + 1 };
        step += 1;
        rs.push_str(&format!(" → {}", num));
    }

    rs.push_str(&format!(",共 {} 步", step));
    rs
}

fn collatz_recursive(integer: &mut u64, inc: &mut i32, output: &mut String) {
    if *inc == 0 {
        output.push_str(&format!("{}", *integer));
    }
    match *integer {
        1 => {
            output.push_str(&format!(",共 {} 步", *inc));
            return;
        }
        _ => {
            match *integer % 2 {
                0 => *integer /= 2,
                _ => *integer = *integer * 3 + 1,
            }
            *inc += 1;
            output.push_str(&format!(" → {}", *integer));
        }
    }

    collatz_recursive(integer, inc, output);
}
