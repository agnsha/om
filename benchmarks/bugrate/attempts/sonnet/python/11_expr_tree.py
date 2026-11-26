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
