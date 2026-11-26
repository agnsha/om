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
