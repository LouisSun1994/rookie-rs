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
