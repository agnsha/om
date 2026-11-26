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
```
### 04_shapes
```python
from dataclasses import dataclass


@dataclass(frozen=True)
class Square:
    side: int


@dataclass(frozen=True)
class Rect:
    width: int
    height: int


@dataclass(frozen=True)
class Triangle:
    base: int
    height: int


type Shape = Square | Rect | Triangle


def area(shape: Shape) -> int:
    match shape:
        case Square(side=s):
            return s * s
        case Rect(width=w, height=h):
            return w * h
        case Triangle(base=b, height=h):
            return b * h // 2


shapes = [Square(3), Rect(2, 5), Triangle(4, 3), Square(1)]
areas = [area(shape) for shape in shapes]
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
        if not self._items:
            return None
        return self._items.pop()

    def peek(self) -> T | None:
        if not self._items:
            return None
        return self._items[-1]

    def __len__(self) -> int:
        return len(self._items)


ints = Stack[int]()
for n in (1, 2, 3):
    ints.push(n)
print(f"len {len(ints)}")
print(f"peek {ints.peek()}")
while (value := ints.pop()) is not None:
    print(f"pop {value}")
print("empty")

words = Stack[str]()
words.push("a")
words.push("b")
print(f"top {words.peek()}")
```
### 06_parse_ages
```python
LINES = ["alice,30", "bob,x", "carol,25", "dave", "erin,-4"]

total = 0
for line in LINES:
    if "," not in line:
        print(f"bad line: {line}")
        continue
    name, _, age_text = line.partition(",")
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
    low, high = 0, len(items) - 1
    while low <= high:
        mid = (low + high) // 2
        if items[mid] == target:
            return mid
        if items[mid] < target:
            low = mid + 1
        else:
            high = mid - 1
    return None


NUMBERS = list(range(1, 20, 2))

for query in (7, 1, 19, 4, 20):
    index = binary_search(NUMBERS, query)
    if index is None:
        print(f"{query} missing")
    else:
        print(f"{query} at {index}")
```
### 08_roman
```python
ROMAN_TABLE = [
    (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"),
    (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
    (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
]


def to_roman(number: int) -> str:
    parts = []
    for value, symbol in ROMAN_TABLE:
        count, number = divmod(number, value)
        parts.append(symbol * count)
    return "".join(parts)


for n in (1994, 2024, 4, 9, 3999, 58):
    print(f"{n} = {to_roman(n)}")
```
### 09_brackets
```python
PAIRS = {")": "(", "]": "[", "}": "{"}


def is_balanced(text: str) -> bool:
    stack: list[str] = []
    for ch in text:
        if ch in "([{":
            stack.append(ch)
        elif ch in PAIRS:
            if not stack or stack.pop() != PAIRS[ch]:
                return False
    return not stack


for text in ("([]{})", "([)]", "((", "{[()()]}", ")("):
    status = "valid" if is_balanced(text) else "invalid"
    print(f"{text} {status}")
```
### 10_primes
```python
def is_prime(n: int) -> bool:
    if n < 2:
        return False
    for d in range(2, int(n ** 0.5) + 1):
        if n % d == 0:
            return False
    return True


primes = [n for n in range(2, 10000) if is_prime(n)]
print(" ".join(str(p) for p in primes[:10]))
print(f"count {len(primes)}")
```
### 11_expr_tree
```python
from dataclasses import dataclass


@dataclass(frozen=True)
class Lit:
    value: int


@dataclass(frozen=True)
class Add:
    left: "Expr"
    right: "Expr"


@dataclass(frozen=True)
class Sub:
    left: "Expr"
    right: "Expr"


@dataclass(frozen=True)
class Mul:
    left: "Expr"
    right: "Expr"


type Expr = Lit | Add | Sub | Mul


def evaluate(expr: Expr) -> int:
    match expr:
        case Lit(value=v):
            return v
        case Add(left=a, right=b):
            return evaluate(a) + evaluate(b)
        case Sub(left=a, right=b):
            return evaluate(a) - evaluate(b)
        case Mul(left=a, right=b):
            return evaluate(a) * evaluate(b)


def depth(expr: Expr) -> int:
    match expr:
        case Lit():
            return 1
        case Add(left=a, right=b) | Sub(left=a, right=b) | Mul(left=a, right=b):
            return 1 + max(depth(a), depth(b))


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
