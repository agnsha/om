enum Shape {
    Square(i32),
    Rect(i32, i32),
    Triangle(i32, i32),
}

impl Shape {
    fn area(&self) -> i32 {
        match *self {
            Shape::Square(side) => side * side,
            Shape::Rect(width, height) => width * height,
            Shape::Triangle(base, height) => base * height / 2,
        }
    }
}

fn main() {
    let shapes = vec![
        Shape::Square(3),
        Shape::Rect(2, 5),
        Shape::Triangle(4, 3),
        Shape::Square(1),
    ];
    let areas: Vec<i32> = shapes.iter().map(|s| s.area()).collect();
    for area in &areas {
        println!("{}", area);
    }
    let total: i32 = areas.iter().sum();
    let largest = areas.iter().max().unwrap();
    println!("total {}", total);
    println!("largest {}", largest);
}
