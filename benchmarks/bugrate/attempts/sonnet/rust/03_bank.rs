struct Account {
    owner: String,
    balance: i64,
}

impl Account {
    fn new(owner: &str, balance: i64) -> Self {
        Account {
            owner: owner.to_string(),
            balance,
        }
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

fn report(result: Result<(), String>, account: &Account) {
    match result {
        Ok(()) => println!("ok {}", account.balance),
        Err(msg) => println!("error: {}", msg),
    }
}

fn main() {
    let mut account = Account::new("ada", 100);

    account.deposit(50);
    report(Ok(()), &account);

    let r = account.withdraw(30);
    report(r, &account);

    let r = account.withdraw(500);
    report(r, &account);

    account.deposit(5);
    report(Ok(()), &account);

    println!("{} has {}", account.owner, account.balance);
}
