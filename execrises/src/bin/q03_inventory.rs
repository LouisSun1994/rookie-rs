use std::fmt;

fn main() {
    // main 建立 capacity 為 10 的玩家，依序撿下面四樣，印出每次的結果：

    // 物品	weight	value	預期
    // sword	4	50	true
    // shield	5	30	true
    // anvil	20	5	false
    // potion	1	80	true
    // 接著印出：total_weight() 是 10、most_valuable() 是 potion、drop_all() 回傳的 Vec 長度是 3。

    let list = vec![
        Item {
            name: ItemName::Sword,
            weight: 4,
            value: 50,
        },
        Item {
            name: ItemName::Shield,
            weight: 5,
            value: 30,
        },
        Item {
            name: ItemName::Anvil,
            weight: 20,
            value: 5,
        },
        Item {
            name: ItemName::Potion,
            weight: 1,
            value: 80,
        },
    ];

    let mut p = Player::new("player", 10);
    println!("{} created, hp: {}", p.name, p.hp);
    for item in list {
        println!(
            "pickup {} result is {}",
            item.name.to_str(),
            match p.pick_up_2(item) {
                Ok(_) => true.to_string(),
                Err(v) => v.to_string(),
            }
        );
        // println!(
        //     "pickup {} result is {}",
        //     item.name.to_str(),
        //     p.pick_up(item)
        // );
    }

    println!("total weight is {}", p.total_weight());
    println!(
        "most valuable is {}",
        p.most_valuable()
            .map_or("null".to_string(), |f| f.value.to_string())
    );
    println!("drop all is {}", p.drop_all().len());
}

#[derive(Debug, Clone)]
enum ItemName {
    Sword,
    Shield,
    Anvil,
    Potion,
}

impl ItemName {
    fn to_str(&self) -> &'static str {
        match self {
            ItemName::Sword => "sword",
            ItemName::Shield => "shield",
            ItemName::Anvil => "anvil",
            ItemName::Potion => "potion",
        }
    }
}

#[derive(Debug, Clone)]
struct Item {
    name: ItemName,
    weight: u32,
    value: u32,
}

impl Drop for Item {
    fn drop(&mut self) {
        println!("drop: {}", self.name.to_str());
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Can't pickup Item: {}", self.name.to_str())
    }
}

struct Player {
    name: String,
    hp: u32,
    capacity: u32, // 背包最多能裝的總重量
    bag: Vec<Item>,
}

impl Player {
    // hp 設 100，背包是空的。
    fn new(name: &str, capacity: u32) -> Self {
        Self {
            name: name.to_string(),
            hp: 100,
            capacity,
            bag: vec![],
        }
    }
    // 背包目前的總重量。
    fn total_weight(&self) -> u32 {
        let mut w = 0;
        for item in &self.bag {
            w += item.weight
        }
        w
    }
    // 放進去之後不超過 capacity 就放進背包並回傳 true，否則不放、回傳 false。
    #[allow(dead_code)]
    fn pick_up(&mut self, item: Item) -> bool {
        if self.capacity - self.total_weight() < item.weight {
            return false;
        }
        self.bag.push(item);
        true
    }
    // q5加分
    #[allow(dead_code)]
    fn pick_up_2(&mut self, item: Item) -> Result<(), Item> {
        if self.capacity - self.total_weight() < item.weight {
            return Err(item);
        }
        self.bag.push(item);
        Ok(())
    }
    // 回傳 value 最高的物品；背包是空的就回傳 None。
    fn most_valuable(&self) -> Option<&Item> {
        let mut max_value_item: Option<&Item> = None;
        for item in &self.bag {
            if max_value_item.is_none() || item.value > max_value_item.unwrap().value {
                max_value_item = Some(item);
            }
        }
        max_value_item
    }
    // 玩家消失，把整個背包交出來。
    fn drop_all(self) -> Vec<Item> {
        self.bag
    }
}
