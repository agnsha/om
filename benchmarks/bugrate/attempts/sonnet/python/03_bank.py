class InsufficientFunds(Exception):
    pass


class Account:
    def __init__(self, owner: str, balance: int = 0) -> None:
        self.owner = owner
        self.balance = balance

    def deposit(self, amount: int) -> None:
        self.balance += amount

    def withdraw(self, amount: int) -> None:
        if amount > self.balance:
            raise InsufficientFunds("insufficient funds")
        self.balance -= amount


def main() -> None:
    account = Account("ada", 100)
    steps = [
        (account.deposit, 50),
        (account.withdraw, 30),
        (account.withdraw, 500),
        (account.deposit, 5),
    ]
    for action, amount in steps:
        try:
            action(amount)
            print(f"ok {account.balance}")
        except InsufficientFunds as e:
            print(f"error: {e}")
    print(f"{account.owner} has {account.balance}")


main()
