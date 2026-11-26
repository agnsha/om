PAIRS = {")": "(", "]": "[", "}": "{"}


def is_balanced(text: str) -> bool:
    stack: list[str] = []
    for ch in text:
        if ch in "([{":
            stack.append(ch)
        elif ch in PAIRS:
            if not stack or stack.pop() != PAIRS[ch]:
                return False
    return not stack


for text in ("([]{})", "([)]", "((", "{[()()]}", ")("):
    status = "valid" if is_balanced(text) else "invalid"
    print(f"{text} {status}")
