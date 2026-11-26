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
