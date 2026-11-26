def binary_search(items: list[int], target: int) -> int | None:
    low, high = 0, len(items) - 1
    while low <= high:
        mid = (low + high) // 2
        if items[mid] == target:
            return mid
        if items[mid] < target:
            low = mid + 1
        else:
            high = mid - 1
    return None


NUMBERS = list(range(1, 20, 2))

for query in (7, 1, 19, 4, 20):
    index = binary_search(NUMBERS, query)
    if index is None:
        print(f"{query} missing")
    else:
        print(f"{query} at {index}")
