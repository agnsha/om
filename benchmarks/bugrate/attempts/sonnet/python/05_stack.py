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
