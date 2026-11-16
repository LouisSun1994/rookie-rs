use std::env;
fn main() {
    let mut num = match env::args().nth(1) {
        Some(arg) => arg.parse::<i32>().unwrap(),
        None => 1,
    };
    let mut step = 0;
    let mut rs = String::new();
    collatz(&mut num, &mut step, &mut rs);
    println!("{}", rs);
}

fn collatz(integer: &mut i32, inc: &mut i32, output: &mut String) {
    if *inc == 0 {
        output.push_str(&format!("{}", *integer));
    }
    match *integer {
        1 => {
            output.push_str(&format!(",共 {} 步", *inc));
            return;
        }
        _ => match *integer % 2 {
            0 => {
                *integer /= 2;
                *inc += 1;
                output.push_str(&format!(" → {}", *integer));
            }
            _ => {
                *integer = *integer * 3 + 1;
                *inc += 1;
            }
        },
    }

    collatz(integer, inc, output);
}
