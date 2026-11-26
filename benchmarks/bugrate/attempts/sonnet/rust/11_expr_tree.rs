enum Expr {
    Lit(i64),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

use Expr::*;

fn lit(n: i64) -> Box<Expr> {
    Box::new(Lit(n))
}

fn eval(e: &Expr) -> i64 {
    match e {
        Lit(n) => *n,
        Add(a, b) => eval(a) + eval(b),
        Sub(a, b) => eval(a) - eval(b),
        Mul(a, b) => eval(a) * eval(b),
    }
}

fn depth(e: &Expr) -> usize {
    match e {
        Lit(_) => 1,
        Add(a, b) | Sub(a, b) | Mul(a, b) => 1 + depth(a).max(depth(b)),
    }
}

fn main() {
    // (2 + 3) * (7 - 4)
    let first = Mul(
        Box::new(Add(lit(2), lit(3))),
        Box::new(Sub(lit(7), lit(4))),
    );
    // 10 - 2 * 3
    let second = Sub(lit(10), Box::new(Mul(lit(2), lit(3))));

    println!("{}", eval(&first));
    println!("{}", eval(&second));
    println!("depth {}", depth(&first));
}
