def long_words(text: str, min_len: int) -> list[str]:
    out = []
    for w in text.split():
        if len(w) >= min_len:
            out.append(w.upper())
    return out


words = long_words("the quick brown fox jumps over the lazy dog", 5)
print("[" + ", ".join(f'"{w}"' for w in words) + "]")
print(f"{len(words)} long words")
