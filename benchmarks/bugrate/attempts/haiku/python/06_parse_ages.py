LINES = ["alice,30", "bob,x", "carol,25", "dave", "erin,-4"]

total = 0
for line in LINES:
    if "," not in line:
        print(f"bad line: {line}")
        continue
    name, _, age_text = line.partition(",")
    try:
        age = int(age_text)
    except ValueError:
        print(f"bad age for {name}")
        continue
    if age < 0:
        print(f"negative age for {name}")
        continue
    print(f"{name} is {age}")
    total += age

print(f"sum {total}")
