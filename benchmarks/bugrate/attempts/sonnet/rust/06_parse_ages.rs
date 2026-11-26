fn main() {
    let lines = ["alice,30", "bob,x", "carol,25", "dave", "erin,-4"];
    let mut sum: i64 = 0;
    for line in lines {
        let Some((name, age_str)) = line.split_once(',') else {
            println!("bad line: {}", line);
            continue;
        };
        match age_str.parse::<i64>() {
            Err(_) => println!("bad age for {}", name),
            Ok(age) if age < 0 => println!("negative age for {}", name),
            Ok(age) => {
                println!("{} is {}", name, age);
                sum += age;
            }
        }
    }
    println!("sum {}", sum);
}
