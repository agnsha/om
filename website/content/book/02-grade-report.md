# 2. A first project: grade report

Before going through the language piece by piece, let's write a whole small
program. Don't worry if parts are unfamiliar; each one is explained in a later
chapter. The goal is to see how Om code feels.

The program takes a list of students and their scores, then prints each
student's letter grade and some statistics.

## The data

Each student has a name and a score. A `struct` groups those two together:

```om
struct Student {
    name: String,
    score: i64,
}
```

`String` is text that the struct owns. `i64` is a 64-bit integer.

## Turning a score into a letter

```om
fn letter(score: i64) -> char {
    match score {
        90..=100 => 'A',
        80..=89 => 'B',
        70..=79 => 'C',
        60..=69 => 'D',
        _ => 'F',
    }
}
```

`match` compares a value against patterns in order. `90..=100` is a range that
includes both ends. `_` matches anything left over. The function's result is
whatever the `match` produces, because the last expression in a function is
its return value.

## Statistics

```om
fn average(students: &Vec[Student]) -> f64 {
    let mut total = 0
    for i in 0..students.len() {
        total += students[i].score
    }
    total as f64 / students.len() as f64
}

fn best(students: &Vec[Student]) -> Option[String] {
    let mut top: Option[String] = None
    let mut top_score = -1
    for i in 0..students.len() {
        let s = &students[i]
        if s.score > top_score {
            top_score = s.score
            top = Some(s.name.clone())
        }
    }
    top
}
```

A few new things:

- `&Vec[Student]` means "borrow a list of students". The function can read the
  list but doesn't take ownership of it, so the caller can keep using it.
- `let mut` declares a variable that can change.
- `students[i]` reads one element; `0..students.len()` counts from zero up to,
  but not including, the length.
- `as f64` converts an integer to a floating-point number.
- `Option[String]` is either `Some(name)` or `None`. Om has no null; an empty
  list has no best student, and the type says so.
- `.clone()` makes a copy of the name, because the list still owns the
  original.

## Putting it together

Here is the complete program. Save it as `grades.om`:

```om
struct Student {
    name: String,
    score: i64,
}

fn letter(score: i64) -> char {
    match score {
        90..=100 => 'A',
        80..=89 => 'B',
        70..=79 => 'C',
        60..=69 => 'D',
        _ => 'F',
    }
}

fn average(students: &Vec[Student]) -> f64 {
    let mut total = 0
    for i in 0..students.len() {
        total += students[i].score
    }
    total as f64 / students.len() as f64
}

fn best(students: &Vec[Student]) -> Option[String] {
    let mut top: Option[String] = None
    let mut top_score = -1
    for i in 0..students.len() {
        let s = &students[i]
        if s.score > top_score {
            top_score = s.score
            top = Some(s.name.clone())
        }
    }
    top
}

fn student(name: str, score: i64) -> Student {
    Student { name: name.to_string(), score }
}

fn main() {
    let class = [
        student("Asha", 93),
        student("Ben", 78),
        student("Chen", 85),
        student("Dana", 59),
    ]

    for s in &class {
        print("{s.name}: {s.score} ({letter(s.score)})")
    }

    print("average: {average(&class)}")

    match best(&class) {
        Some(name) => print("top of the class: {name}"),
        None => print("no students"),
    }
}
```

```console
$ om run grades.om
Asha: 93 (A)
Ben: 78 (C)
Chen: 85 (B)
Dana: 59 (F)
average: 78.75
top of the class: Asha
```

## Try it

- Add a student and run it again.
- Make `letter` give `'A'` for 95 and up and `'B'` for 85 to 94, then check
  that the compiler still accepts the `match`. What happens if you delete the
  `_` arm?
- Write a `count_passing` function that returns how many students scored 60 or
  more.

Next: [Common concepts](03-common-concepts.md).
