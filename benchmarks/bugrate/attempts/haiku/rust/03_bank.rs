struct Account {
    owner: String,
    balance: i64,
}

impl Account {
    fn new(owner: &str, balance: i64) -> Self {
        Account { owner: owner.to_string(), balance }
    }

    fn deposit(&mut self, amount: i64) {
        self.balance += amount;
    }

    fn withdraw(&mut self, amount: i64) -> Result<(), String> {
        if amount > self.balance {
            return Err("insufficient funds".to_string());
        }
        self.balance -= amount;
        Ok(())
    }
}

fn main() {
    let mut account = Account::new("ada", 100);

    account.deposit(50);
    println!("ok {}", account.balance);

    match account.withdraw(30) {
        Ok(()) => println!("ok {}", account.balance),
        Err(e) => println!("error: {}", e),
    }
    match account.withdraw(500) {
        Ok(()) => println!("ok {}", account.balance),
        Err(e) => println!("error: {}", e),
    }
    account.deposit(5);
    println!("ok {}", account.balance);

    println!("{} has {}", account.owner, account.balance);
}
