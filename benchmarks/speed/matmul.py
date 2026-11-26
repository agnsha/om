import sys

n = int(sys.argv[1])
a = [0.0] * (n * n)
b = [0.0] * (n * n)
c = [0.0] * (n * n)
for i in range(n):
    for j in range(n):
        a[i * n + j] = (i + j) / n
        b[i * n + j] = (i - j) / n
for i in range(n):
    for k in range(n):
        x = a[i * n + k]
        for j in range(n):
            c[i * n + j] += x * b[k * n + j]
total = 0.0
for v in c:
    total += v
print(total)
