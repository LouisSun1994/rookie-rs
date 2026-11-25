use std::fmt;

fn main() {
    // 3 4 +	7
    // 5 1 2 + 4 * + 3 -	14
    // 2 3 4 * +	14
    // 7 2 /	3
    // 10 3 -	7
    // 1 +	StackUnderflow
    // 4 0 /	DivideByZero
    // 1 2	TooManyOperands(2)
    // （空字串）	Empty
    // 2 x +	BadToken("x")
    // 9223372036854775807 1 +	Overflow
    // -9223372036854775808 -1 /	Overflow

    let list = vec![
        "3 4 +",
        "5 1 2 + 4 * + 3 -",
        "2 3 4 * +",
        "7 2 /",
        "10 3 -",
        "1 +",
        "4 0 /",
        "1 2",
        "",
        "2 x +",
        "9223372036854775807 1 +",
        "-9223372036854775808 -1 /",
        "1 +",
    ];
    for li in list {
        match eval(li) {
            Ok(v) => println!("{li} => {v}"),
            Err(e) => {
                let question = if li.is_empty() { "(空字串)" } else { li };
                println!("{question} => {e}")
            }
        };
    }
}

#[derive(Debug)]
enum CalcError {
    Empty,            // 運算式裡沒有任何東西
    BadToken(String), // 不是數字也不是運算子，帶著那個 token
    StackUnderflow,   // 運算子需要兩個數字，但 stack 裡不夠
    DivideByZero,
    Overflow,               // 運算結果超出 i64
    TooManyOperands(usize), // 算完之後 stack 裡剩不只一個數字，帶著剩幾個
}

fn pop2(stack: &mut Vec<i64>) -> Result<(i64, i64), CalcError> {
    let Some(addend) = stack.pop() else {
        return Err(CalcError::StackUnderflow);
    };
    let Some(augend) = stack.pop() else {
        return Err(CalcError::StackUnderflow);
    };

    Ok((addend, augend))
}

fn eval(expr: &str) -> Result<i64, CalcError> {
    let mut num_stack: Vec<i64> = Vec::new();

    for token in expr.split_whitespace() {
        match Op::parse(token) {
            Some(op) => {
                let (addend, augend) = pop2(&mut num_stack)?;
                num_stack.push(Op::calc(&op, augend, addend)?)
            }
            None => {
                if let Ok(iteger) = token.parse::<i64>() {
                    num_stack.push(iteger);
                } else {
                    return Err(CalcError::BadToken(token.to_string()));
                }
            }
        }
    }

    if num_stack.len() > 1 {
        return Err(CalcError::TooManyOperands(num_stack.len()));
    }

    let rs = num_stack.pop().ok_or(CalcError::Empty)?;
    Ok(rs)
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalcError::Empty => write!(f, "Empty"),
            CalcError::BadToken(t) => write!(f, "BadToken(\"{t}\")"),
            CalcError::StackUnderflow => write!(f, "StackUnderflow"),
            CalcError::DivideByZero => write!(f, "DivideByZero"),
            CalcError::Overflow => write!(f, "Overflow"),
            CalcError::TooManyOperands(n) => write!(f, "TooManyOperands({n})"),
        }
    }
}

enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    fn parse(token: &str) -> Option<Op> {
        match token {
            "+" => Some(Op::Add),
            "-" => Some(Op::Sub),
            "*" => Some(Op::Mul),
            "/" => Some(Op::Div),
            _ => None,
        }
    }
    fn calc(&self, augend: i64, addend: i64) -> Result<i64, CalcError> {
        match *self {
            Op::Add => augend.checked_add(addend).ok_or(CalcError::Overflow),
            Op::Sub => augend.checked_sub(addend).ok_or(CalcError::Overflow),
            Op::Mul => augend.checked_mul(addend).ok_or(CalcError::Overflow),
            Op::Div => {
                if addend == 0 {
                    return Err(CalcError::DivideByZero);
                }
                augend.checked_div(addend).ok_or(CalcError::Overflow)
            }
        }
    }
}
