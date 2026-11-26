enum Expr {
    Lit(i64),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

impl Expr {
    fn eval(&self) -> i64 {
        match self {
            Expr::Lit(n) => *n,
            Expr::Add(a, b) => a.eval() + b.eval(),
            Expr::Sub(a, b) => a.eval() - b.eval(),
            Expr::Mul(a, b) => a.eval() * b.eval(),
        }
    }

    fn depth(&self) -> usize {
        match self {
            Expr::Lit(_) => 1,
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) => 1 + a.depth().max(b.depth()),
        }
    }
}

fn main() {
    // (2 + 3) * (7 - 4)
    let first = Expr::Mul(
        Box::new(Expr::Add(Box::new(Expr::Lit(2)), Box::new(Expr::Lit(3)))),
        Box::new(Expr::Sub(Box::new(Expr::Lit(7)), Box::new(Expr::Lit(4)))),
    );
    // 10 - 2 * 3
    let second = Expr::Sub(
        Box::new(Expr::Lit(10)),
        Box::new(Expr::Mul(Box::new(Expr::Lit(2)), Box::new(Expr::Lit(3)))),
    );

    println!("{}", first.eval());
    println!("{}", second.eval());
    println!("depth {}", first.depth());
}
