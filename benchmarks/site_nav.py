"""Add a Benchmarks link to the header and sidebar of every page of the built
site. Safe to run twice. Usage: python3 site_nav.py path/to/dist"""

import os
import sys

root = sys.argv[1]
for d, _, files in os.walk(root):
    for f in files:
        if not f.endswith(".html"):
            continue
        p = os.path.join(d, f)
        s = open(p, encoding="utf-8").read()
        if "benchmarks.html" in s:
            continue
        up = "../" * os.path.relpath(p, root).count(os.sep)
        s = s.replace(f'<a href="{up}games.html">Games</a>', f'<a href="{up}games.html">Games</a>\n    <a href="{up}benchmarks.html">Benchmarks</a>', 1)
        s = s.replace(f'<li><a href="{up}tools.html"', f'<li><a href="{up}benchmarks.html">Benchmarks</a></li><li><a href="{up}tools.html"', 1)
        open(p, "w", encoding="utf-8").write(s)
