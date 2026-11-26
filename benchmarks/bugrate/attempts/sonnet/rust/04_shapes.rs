enum Shape {
    Square(i64),
    Rect(i64, i64),
    Triangle(i64, i64),
}

impl Shape {
    fn area(&self) -> i64 {
        match self {
            Shape::Square(s) => s * s,
            Shape::Rect(w, h) => w * h,
            Shape::Triangle(b, h) => b * h / 2,
        }
    }
}

fn main() {
    let shapes = [
        Shape::Square(3),
        Shape::Rect(2, 5),
        Shape::Triangle(4, 3),
        Shape::Square(1),
    ];
    let areas: Vec<i64> = shapes.iter().map(|s| s.area()).collect();
    for a in &areas {
        println!("{}", a);
    }
    let total: i64 = areas.iter().sum();
    let largest = areas.iter().max().unwrap();
    println!("total {}", total);
    println!("largest {}", largest);
}
