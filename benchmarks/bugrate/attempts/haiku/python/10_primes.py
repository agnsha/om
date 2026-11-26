def is_prime(n: int) -> bool:
    if n < 2:
        return False
    for d in range(2, int(n ** 0.5) + 1):
        if n % d == 0:
            return False
    return True


primes = [n for n in range(2, 10000) if is_prime(n)]
print(" ".join(str(p) for p in primes[:10]))
print(f"count {len(primes)}")
