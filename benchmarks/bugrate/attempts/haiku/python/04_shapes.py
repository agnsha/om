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
