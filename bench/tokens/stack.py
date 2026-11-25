from typing import Generic, TypeVar

T = TypeVar("T")


class Stack(Generic[T]):
    def __init__(self) -> None:
        self.items: list[T] = []

    def push(self, x: T) -> None:
        self.items.append(x)

    def pop(self) -> T | None:
        return self.items.pop() if self.items else None

    def __len__(self) -> int:
        return len(self.items)


s: Stack[str] = Stack()
s.push("a")
s.push("b")
print(f'Some("{s.pop()}")')
print(len(s))
