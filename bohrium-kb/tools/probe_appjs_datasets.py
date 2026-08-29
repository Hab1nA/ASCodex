#!/usr/bin/env python3
"""Fetch js/app.js and extract dataset download logic."""
import urllib.request, re, os

OUT = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"
UA = {"User-Agent": "Mozilla/5.0"}
body = urllib.request.urlopen(urllib.request.Request("https://play.bohrium.com/js/app.js?v=202608160044", headers=UA), timeout=180).read().decode("utf-8", "replace")
open(os.path.join(OUT, "appjs_main.js"), "w", encoding="utf-8").write(body)
print("app.js chars:", len(body))

# find all URL fragments containing 'dataset'
hits = set(re.findall(r"[\"'`]([^\"'`]{0,80}dataset[^\"'`]{0,80})[\"'`]", body, re.I))
for h in sorted(hits):
    print("D:", h)

# find fetch/api helper calls with template literals around 'datasets'
for m in re.finditer(r"datasets", body, re.I):
    s = max(0, m.start() - 160)
    seg = body[s:m.end() + 160].replace("\n", " ")
    print("CTX:", seg)
    print("---")
