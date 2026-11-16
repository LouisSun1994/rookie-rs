// ===== q01 觀察與回答 =====
//
// 【實驗】把參數型別改成 u32，各跑一次
//   debug   ：
//    Compiling execrises v0.1.0 (/Users/admin/rs/rookie-rs/execrises)
//     Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
//      Running `target/debug/q01_collatz 3`
// recursive 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
// loop 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
//   release ：
//     Finished `release` profile [optimized] target(s) in 0.04s
//      Running `target/release/q01_collatz 3`
// recursive 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
// loop 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1,共 7 步
//
//   [複盤] 輸入 3 的路徑最大只到 16，沒有超出 u32，所以兩種 build 看不出差別。
//          要選一個「路徑中途會超過 u32 上限（約 42.9 億）」的輸入，例如 159487：
//
//            debug   ：panicked at ...: attempt to multiply with overflow
//                      → 在溢位的那一步立刻停下，並指出是哪一行
//            release ：沒有任何錯誤，正常印出路徑，結尾是「共 245 步」
//                      → 但正確答案（u64）是 183 步。乘法結果被截斷成 32 bits，
//                        程式拿著錯的數字繼續算，剛好也走到 1，於是安靜地給出錯的答案
//
//          release 的後果不固定，取決於截斷後的數字走去哪：
//            - 剛好走到 1        → 安靜的錯誤答案（上面這個例子）
//            - 掉進到不了 1 的循環 → while 版永遠跑不完；遞迴版 stack overflow
//              （先前 i32 + 遞迴版 + 輸入 i32::MAX 就是這種）
//
// 【問題 1】兩種 build 行為為什麼不同？
//           那個「檢查」是誰放進去的（CPU / kernel / std / rustc）？什麼時間點放的？
//   預測：rustc compile 檢查
//   跑完後：
//
//   [複盤] 「誰」答對了：rustc。要修的是「compile 檢查」這四個字，
//          它把兩個不同的時間點合在一起了：
//
//            編譯期：rustc「放進」檢查。在乘法指令後面多產生幾條機器碼：
//                    「看 CPU 的溢位旗標，有溢位就跳去 panic」
//            執行期：CPU「執行」那幾條機器碼，檢查才真的發生
//
//          證據：溢位與否取決於你從命令列輸入的數字，編譯當下 rustc 不可能知道，
//                所以檢查不可能在編譯期完成，只能是編譯期預先埋好、執行期才跑。
//
//          兩種 build 的差別就是 rustc 有沒有埋那幾條指令：
//            dev profile     → overflow-checks = true  → 有埋 → 溢位時 panic
//            release profile → overflow-checks = false → 沒埋 → 溢位時截斷，繼續跑
//          這個設定由 cargo 依 profile 決定，再當成參數傳給 rustc。
//          kernel 和 std 都沒有參與；CPU 只是照著機器碼執行。
//
// 【問題 2】把記錄最佳值的變數的 mut 拿掉會怎樣？錯誤出現在編譯期還是執行期？
//   預測：compile fail
//   跑完後：
//
//   [複盤] 對。錯誤訊息是：
//            error[E0384]: cannot assign twice to immutable variable `best`
//          這個檢查完全在編譯期完成（rustc 只看程式碼就能判斷，不需要知道輸入），
//          編不過就沒有執行檔，執行期沒有任何對應的檢查或成本。
//
//          和問題 1 對照：
//            mut 檢查  ：編譯期檢查，編譯期報錯，執行期零成本
//            溢位檢查  ：編譯期埋指令，執行期檢查，執行期 panic，有少量成本
//                        （所以 release 預設把它關掉）

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
