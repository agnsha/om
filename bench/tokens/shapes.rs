#[derive(Debug)]
enum Shape {
    Circle(f64),
    Rect(f64, f64),
}

trait Area {
    fn area(&self) -> f64;
}

impl Area for Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle(r) => 3.0 * r * r,
            Shape::Rect(w, h) => w * h,
        }
    }
}

fn main() {
    let shapes = vec![Shape::Circle(1.0), Shape::Rect(2.0, 3.5)];
    let mut total = 0.0;
    for s in &shapes {
        println!("{:?}: {}", s, s.area());
        total += s.area();
    }
    println!("total {}", total);
}
