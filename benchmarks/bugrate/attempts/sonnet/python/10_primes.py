def sieve(limit: int) -> list[int]:
    is_prime = [True] * limit
    is_prime[0:2] = [False, False]
    for i in range(2, int(limit ** 0.5) + 1):
        if is_prime[i]:
            for j in range(i * i, limit, i):
                is_prime[j] = False
    return [i for i, p in enumerate(is_prime) if p]


primes = sieve(10000)
print(" ".join(str(p) for p in primes[:10]))
print(f"count {len(primes)}")
