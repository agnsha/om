from collections import Counter

text = "the quick brown fox jumps over the lazy dog the fox"
counts = Counter(text.split(" "))
for word, count in counts.items():
    print(f"{word}: {count}")
