def parse_all(items: list[str]) -> int:
    total = 0
    for s in items:
        try:
            total += int(s.strip())
        except ValueError as e:
            raise ValueError(f"item `{s}`: {e}") from e
    return total


for items in (["1", " 2", "39"], ["4", "x5"]):
    try:
        print(f"Ok({parse_all(items)})")
    except ValueError as e:
        print(f'Err("{e}")')
