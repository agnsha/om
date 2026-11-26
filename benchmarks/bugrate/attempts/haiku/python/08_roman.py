ROMAN_TABLE = [
    (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"),
    (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
    (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
]


def to_roman(number: int) -> str:
    parts = []
    for value, symbol in ROMAN_TABLE:
        count, number = divmod(number, value)
        parts.append(symbol * count)
    return "".join(parts)


for n in (1994, 2024, 4, 9, 3999, 58):
    print(f"{n} = {to_roman(n)}")
