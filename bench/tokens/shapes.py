from dataclasses import dataclass


@dataclass
class Circle:
    r: float

    def area(self) -> float:
        return 3.0 * self.r * self.r


@dataclass
class Rect:
    w: float
    h: float

    def area(self) -> float:
        return self.w * self.h


shapes = [Circle(1.0), Rect(2.0, 3.5)]
total = 0.0
for s in shapes:
    print(f"{s}: {s.area():g}")
    total += s.area()
print(f"total {total:g}")
