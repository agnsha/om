### 01_fizzbuzz
```python
for n in range(1, 16):
    if n % 15 == 0:
        print("FizzBuzz")
    elif n % 3 == 0:
        print("Fizz")
    elif n % 5 == 0:
        print("Buzz")
    else:
        print(n)
```

### 02_word_count
```python
from collections import Counter

text = "the quick brown fox jumps over the lazy dog the fox"
counts = Counter(text.split(" "))
for word, count in counts.items():
    print(f"{word}: {count}")
```

### 03_bank
```python
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
```

### 04_shapes
```python
from dataclasses import dataclass


@dataclass
class Square:
    side: int


@dataclass
class Rect:
    width: int
    height: int


@dataclass
class Triangle:
    base: int
    height: int


Shape = Square | Rect | Triangle


def area(shape: Shape) -> int:
    match shape:
        case Square(side):
            return side * side
        case Rect(width, height):
            return width * height
        case Triangle(base, height):
            return base * height // 2


shapes = [Square(3), Rect(2, 5), Triangle(4, 3), Square(1)]
areas = [area(s) for s in shapes]
for a in areas:
    print(a)
print(f"total {sum(areas)}")
print(f"largest {max(areas)}")
```

### 05_stack
```python
class Stack[T]:
    def __init__(self) -> None:
        self._items: list[T] = []

    def push(self, item: T) -> None:
        self._items.append(item)

    def pop(self) -> T | None:
        return self._items.pop() if self._items else None

    def peek(self) -> T | None:
        return self._items[-1] if self._items else None

    def __len__(self) -> int:
        return len(self._items)


ints: Stack[int] = Stack()
for n in (1, 2, 3):
    ints.push(n)
print(f"len {len(ints)}")
print(f"peek {ints.peek()}")
while (value := ints.pop()) is not None:
    print(f"pop {value}")
print("empty")

strings: Stack[str] = Stack()
strings.push("a")
strings.push("b")
print(f"top {strings.peek()}")
```

### 06_parse_ages
```python
lines = ["alice,30", "bob,x", "carol,25", "dave", "erin,-4"]

total = 0
for line in lines:
    if "," not in line:
        print(f"bad line: {line}")
        continue
    name, age_text = line.split(",", 1)
    try:
        age = int(age_text)
    except ValueError:
        print(f"bad age for {name}")
        continue
    if age < 0:
        print(f"negative age for {name}")
        continue
    print(f"{name} is {age}")
    total += age
print(f"sum {total}")
```

### 07_binary_search
```python
def binary_search(items: list[int], target: int) -> int | None:
    lo, hi = 0, len(items) - 1
    while lo <= hi:
        mid = (lo + hi) // 2
        if items[mid] == target:
            return mid
        if items[mid] < target:
            lo = mid + 1
        else:
            hi = mid - 1
    return None


data = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19]
for query in (7, 1, 19, 4, 20):
    index = binary_search(data, query)
    if index is None:
        print(f"{query} missing")
    else:
        print(f"{query} at {index}")
```

### 08_roman
```python
NUMERALS = [
    (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"),
    (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
    (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
]


def to_roman(n: int) -> str:
    parts = []
    for value, symbol in NUMERALS:
        count, n = divmod(n, value)
        parts.append(symbol * count)
    return "".join(parts)


for number in (1994, 2024, 4, 9, 3999, 58):
    print(f"{number} = {to_roman(number)}")
```

### 09_brackets
```python
PAIRS = {")": "(", "]": "[", "}": "{"}


def is_balanced(text: str) -> bool:
    stack = []
    for ch in text:
        if ch in "([{":
            stack.append(ch)
        elif ch in PAIRS:
            if not stack or stack.pop() != PAIRS[ch]:
                return False
    return not stack


for s in ["([]{})", "([)]", "((", "{[()()]}", ")("]:
    print(f"{s} {'valid' if is_balanced(s) else 'invalid'}")
```

### 10_primes
```python
def sieve(limit: int) -> list[int]:
    is_prime = [True] * limit
    is_prime[0:2] = [False, False]
    for i in range(2, int(limit ** 0.5) + 1):
        if is_prime[i]:
            for j in range(i * i, limit, i):
                is_prime[j] = False
    return [i for i, p in enumerate(is_prime) if p]


primes = sieve(10000)
print(" ".join(str(p) for p in primes[:10]))
print(f"count {len(primes)}")
```

### 11_expr_tree
```python
from dataclasses import dataclass


@dataclass
class Lit:
    value: int


@dataclass
class Add:
    left: "Expr"
    right: "Expr"


@dataclass
class Sub:
    left: "Expr"
    right: "Expr"


@dataclass
class Mul:
    left: "Expr"
    right: "Expr"


Expr = Lit | Add | Sub | Mul


def evaluate(e: Expr) -> int:
    match e:
        case Lit(value):
            return value
        case Add(l, r):
            return evaluate(l) + evaluate(r)
        case Sub(l, r):
            return evaluate(l) - evaluate(r)
        case Mul(l, r):
            return evaluate(l) * evaluate(r)


def depth(e: Expr) -> int:
    match e:
        case Lit():
            return 1
        case Add(l, r) | Sub(l, r) | Mul(l, r):
            return 1 + max(depth(l), depth(r))


first = Mul(Add(Lit(2), Lit(3)), Sub(Lit(7), Lit(4)))
second = Sub(Lit(10), Mul(Lit(2), Lit(3)))

print(evaluate(first))
print(evaluate(second))
print(f"depth {depth(first)}")
```

### 12_caesar
```python
def shift(text: str, k: int) -> str:
    out = []
    for ch in text:
        if "a" <= ch <= "z":
            out.append(chr((ord(ch) - ord("a") + k) % 26 + ord("a")))
        elif "A" <= ch <= "Z":
            out.append(chr((ord(ch) - ord("A") + k) % 26 + ord("A")))
        else:
            out.append(ch)
    return "".join(out)


encoded = shift("Hello, World!", 3)
print(encoded)
print(shift(encoded, 23))
print(shift("xyz ABC", 4))
```
