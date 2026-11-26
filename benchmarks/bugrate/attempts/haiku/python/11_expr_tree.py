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
