from dataclasses import dataclass


@dataclass
class Account:
    owner: str
    balance: int = 0

    def deposit(self, amount: int) -> None:
        self.balance += amount

    def withdraw(self, amount: int) -> None:
        if amount > self.balance:
            raise ValueError("insufficient funds")
        self.balance -= amount


account = Account("ada", 100)
steps = [
    (account.deposit, 50),
    (account.withdraw, 30),
    (account.withdraw, 500),
    (account.deposit, 5),
]
for step, amount in steps:
    try:
        step(amount)
    except ValueError as err:
        print(f"error: {err}")
    else:
        print(f"ok {account.balance}")
print(f"{account.owner} has {account.balance}")
