You are writing small programs for a benchmark. Write each task below as one
complete, runnable Python program.

Rules:
- A single file, Python 3.12, standard library only.
- Each program has no input and must print exactly the lines the task asks for.
- Write idiomatic, normal code, the way you would for a colleague.
- Do not use any tools, do not read or write files, and do not run anything.
  Answer from what you know.
- Reply with only the programs: for each task, a line `### <task id>` followed
  by one fenced code block. No other text.

# Tasks

## 01_fizzbuzz
For each number from 1 to 15, print `FizzBuzz` if it is divisible by 15, `Fizz`
if divisible by 3, `Buzz` if divisible by 5, and the number otherwise. One per line.

## 02_word_count
Count the words in the text `the quick brown fox jumps over the lazy dog the fox`
(split on spaces). Print each distinct word once, in order of first appearance,
as `word: count`.

## 03_bank
Write an `Account` type with an owner name and an integer balance, a `deposit`
method, and a `withdraw` method that fails with the error message
`insufficient funds` instead of going below zero. Starting from an account for
`ada` with balance 100, perform: deposit 50, withdraw 30, withdraw 500, deposit 5.
After each step print `ok <balance>` or `error: <message>`. Finally print
`ada has <balance>`.

## 04_shapes
Define shapes as an enum: `Square(side)`, `Rect(width, height)` and
`Triangle(base, height)`, all integers. The area of a triangle is
`base * height / 2` with integer division. For the list Square(3), Rect(2, 5),
Triangle(4, 3), Square(1), print each area on its own line, then
`total <sum>`, then `largest <largest area>`.

## 05_stack
Write a generic stack type with `push`, `pop` (returns nothing when empty),
`peek` and `len`. Push 1, 2, 3 onto an integer stack, then print `len 3`,
`peek 3`, and pop until empty, printing `pop <value>` for each, then print
`empty` when pop returns nothing. Then push the strings `a` and `b` onto a
string stack and print `top b`.

## 06_parse_ages
Parse these lines, each `name,age`: `alice,30`, `bob,x`, `carol,25`, `dave`,
`erin,-4`. For a valid line print `<name> is <age>`. For an age that is not an
integer print `bad age for <name>`. For a line without a comma print
`bad line: <line>`. For a negative age print `negative age for <name>`. Finally
print `sum <sum of valid ages>`.

## 07_binary_search
Implement binary search over the sorted list 1, 3, 5, 7, 9, 11, 13, 15, 17, 19.
For each query 7, 1, 19, 4, 20, print `<query> at <index>` if found or
`<query> missing` if not.

## 08_roman
Convert each of 1994, 2024, 4, 9, 3999, 58 to a Roman numeral and print
`<number> = <roman>`.

## 09_brackets
For each of the strings `([]{})`, `([)]`, `((`, `{[()()]}`, `)(`, print
`<string> valid` or `<string> invalid` depending on whether the brackets
(), [] and {} are balanced and properly nested.

## 10_primes
Print the first 10 prime numbers on one line separated by single spaces, then
print `count <n>` where n is the number of primes below 10000.

## 11_expr_tree
Represent arithmetic expressions as a recursive tree type with integer
literals and Add, Sub and Mul nodes. Build the tree for `(2 + 3) * (7 - 4)` and
the tree for `10 - 2 * 3` (usual precedence), then print the value of each, one
per line. Then print the depth of the first tree as `depth <d>`, where a literal
has depth 1.

## 12_caesar
Write `shift(text, k)` that rotates ASCII letters by k places (keeping case and
wrapping around) and leaves other characters alone. Print
`shift("Hello, World!", 3)`, then `shift` of that result by 23 (which undoes
it), then `shift("xyz ABC", 4)`.
