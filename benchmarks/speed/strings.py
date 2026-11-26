import sys

n = int(sys.argv[1])
parts = []
for i in range(n):
    parts.append(f"{i},")
s = "".join(parts)
print(len(s), s.count("7"))
