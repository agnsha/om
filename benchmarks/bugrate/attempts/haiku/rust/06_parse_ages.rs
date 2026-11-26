fn main() {
    let lines = ["alice,30", "bob,x", "carol,25", "dave", "erin,-4"];
    let mut sum: i64 = 0;

    for line in lines {
        let Some((name, age_text)) = line.split_once(',') else {
            println!("bad line: {}", line);
            continue;
        };
        match age_text.parse::<i64>() {
            Ok(age) if age < 0 => println!("negative age for {}", name),
            Ok(age) => {
                println!("{} is {}", name, age);
                sum += age;
            }
            Err(_) => println!("bad age for {}", name),
        }
    }

    println!("sum {}", sum);
}
