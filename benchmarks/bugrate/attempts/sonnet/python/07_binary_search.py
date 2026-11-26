def binary_search(items: list[int], target: int) -> int | None:
    lo, hi = 0, len(items) - 1
    while lo <= hi:
        mid = (lo + hi) // 2
        if items[mid] == target:
            return mid
        if items[mid] < target:
            lo = mid + 1
        else:
            hi = mid - 1
    return None


data = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19]
for query in (7, 1, 19, 4, 20):
    index = binary_search(data, query)
    if index is None:
        print(f"{query} missing")
    else:
        print(f"{query} at {index}")
