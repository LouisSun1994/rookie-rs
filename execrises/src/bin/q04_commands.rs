fn main() {
    let lines = [
        "add sword 2",
        "add potion 5",
        "add sword 1",
        "list",
        "remove potion",
        "remove dragon",
        "fly away",
        "add shield",
        "add shield many",
        "list",
        "clear",
        "list",
        "quit",
        "add ghost 1",
    ];

    let mut bag = Vec::new();
    for line in lines {
        println!("> {line}");
        // let cmd = Command::parse(line);
        // let rs = match cmd {
        //     Some(cc) => Command::execute(cc, &mut bag),
        //     None => {
        //         println!("無法解析");
        //         true
        //     }
        // };

        // if !rs {
        //     break;
        // }

        let Some(cmd) = Command::parse(line) else {
            println!("無法解析");
            continue;
        };
        if !Command::execute(cmd, &mut bag) {
            break;
        }
    }
}

enum Command {
    Add(String, u32), // 物品名稱、數量
    Remove(String),
    List,
    Quit,
    Clear,
}

impl Command {
    fn parse(line: &str) -> Option<Command> {
        match line.split_whitespace().collect::<Vec<&str>>().as_slice() {
            ["add", item, count] if let Ok(count_32) = count.parse::<u32>() => {
                Some(Command::Add(String::from(*item), count_32))
            }
            ["remove", item] => Some(Command::Remove(String::from(*item))),
            ["list"] => Some(Command::List),
            ["quit"] => Some(Command::Quit),
            ["clear"] => Some(Command::clear()),
            _ => None,
        }
    }
    fn execute(cmd: Command, bag: &mut Vec<(String, u32)>) -> bool {
        let rs = match cmd {
            Command::Add(item, count) => {
                // println!("{item} x{count}");
                match bag.iter_mut().find(|f| f.0 == item) {
                    Some((_, quantity)) => *quantity += count,
                    None => bag.push((item, count)),
                }
                true
            }
            Command::Remove(item) => {
                match bag.iter().position(|f| f.0 == item) {
                    Some(idx) => {
                        // println!("remove {item}");
                        bag.remove(idx);
                    }
                    None => println!("找不到: {item}"),
                }
                true
            }
            Command::List => {
                bag.iter().for_each(|(k, v)| println!("{k} x{v}"));
                true
            }
            Command::Quit => false,
            Command::Clear => {
                bag.clear();
                true
            }
        };
        rs
    }
    fn clear() -> Command {
        Command::Clear
    }
}
