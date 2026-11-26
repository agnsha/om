# Reference outputs, written in Python by hand. Writes ../tasks/<task>.expected.
import io, os, contextlib

def t01():
    for i in range(1, 16):
        print("FizzBuzz" if i % 15 == 0 else "Fizz" if i % 3 == 0 else "Buzz" if i % 5 == 0 else i)

def t02():
    c = {}
    for w in "the quick brown fox jumps over the lazy dog the fox".split(" "):
        c[w] = c.get(w, 0) + 1
    for w, n in c.items():
        print(f"{w}: {n}")

def t03():
    b = 100
    for op, n in [("d", 50), ("w", 30), ("w", 500), ("d", 5)]:
        if op == "w" and n > b:
            print("error: insufficient funds")
            continue
        b += n if op == "d" else -n
        print(f"ok {b}")
    print(f"ada has {b}")

def t04():
    areas = [9, 10, 6, 1]
    for a in areas:
        print(a)
    print(f"total {sum(areas)}")
    print(f"largest {max(areas)}")

def t05():
    print("len 3"); print("peek 3")
    for v in (3, 2, 1):
        print(f"pop {v}")
    print("empty"); print("top b")

def t06():
    s = 0
    for line in ["alice,30", "bob,x", "carol,25", "dave", "erin,-4"]:
        if "," not in line:
            print(f"bad line: {line}"); continue
        name, age = line.split(",", 1)
        try:
            a = int(age)
        except ValueError:
            print(f"bad age for {name}"); continue
        if a < 0:
            print(f"negative age for {name}"); continue
        print(f"{name} is {a}"); s += a
    print(f"sum {s}")

def t07():
    xs = list(range(1, 20, 2))
    for q in (7, 1, 19, 4, 20):
        print(f"{q} at {xs.index(q)}" if q in xs else f"{q} missing")

def t08():
    vals = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")]
    for n in (1994, 2024, 4, 9, 3999, 58):
        m, r = n, ""
        for v, s in vals:
            while m >= v:
                r += s; m -= v
        print(f"{n} = {r}")

def t09():
    pairs = {")": "(", "]": "[", "}": "{"}
    for s in ["([]{})", "([)]", "((", "{[()()]}", ")("]:
        st, ok = [], True
        for ch in s:
            if ch in "([{": st.append(ch)
            elif not st or st.pop() != pairs[ch]: ok = False; break
        ok = ok and not st
        print(f"{s} {'valid' if ok else 'invalid'}")

def t10():
    def isp(n): return n > 1 and all(n % d for d in range(2, int(n ** .5) + 1))
    ps = [n for n in range(2, 100) if isp(n)][:10]
    print(" ".join(map(str, ps)))
    print(f"count {sum(1 for n in range(10000) if isp(n))}")

def t11():
    print(15); print(4); print("depth 3")

def t12():
    def shift(t, k):
        o = ""
        for c in t:
            if "a" <= c <= "z": o += chr((ord(c) - 97 + k) % 26 + 97)
            elif "A" <= c <= "Z": o += chr((ord(c) - 65 + k) % 26 + 65)
            else: o += c
        return o
    a = shift("Hello, World!", 3)
    print(a); print(shift(a, 23)); print(shift("xyz ABC", 4))

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tasks")
names = ["01_fizzbuzz", "02_word_count", "03_bank", "04_shapes", "05_stack", "06_parse_ages", "07_binary_search", "08_roman", "09_brackets", "10_primes", "11_expr_tree", "12_caesar"]
for i, n in enumerate(names, 1):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        globals()[f"t{i:02d}"]()
    open(os.path.join(out, n + ".expected"), "w").write(buf.getvalue())
