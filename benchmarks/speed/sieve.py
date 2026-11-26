import sys

n = int(sys.argv[1])
composite = [False] * (n + 1)
count = 0
i = 2
while i <= n:
    if not composite[i]:
        count += 1
        j = i * i
        while j <= n:
            composite[j] = True
            j += i
    i += 1
print(count)
