import sys

n = int(sys.argv[1])
v = []
seed = 42
for _ in range(n):
    seed = (seed * 1103515245 + 12345) % 2147483648
    v.append(seed)
v.sort()
check = 0
for i in range(0, len(v), 1000):
    check = (check + v[i]) % 1000000007
print(check)
