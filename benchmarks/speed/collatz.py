import sys

n = int(sys.argv[1])
best = 0
best_len = 0
for start in range(1, n):
    x = start
    length = 1
    while x != 1:
        x = x // 2 if x % 2 == 0 else 3 * x + 1
        length += 1
    if length > best_len:
        best_len = length
        best = start
print(best, best_len)
