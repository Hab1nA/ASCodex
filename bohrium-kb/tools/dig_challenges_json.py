#!/usr/bin/env python3
"""Inspect /data/challenges.json entry for the wake challenge: any data urls?"""
import urllib.request, json, os, re

OUT = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"
UA = {"User-Agent": "Mozilla/5.0"}
raw = urllib.request.urlopen(urllib.request.Request("https://play.bohrium.com/data/challenges.json", headers=UA), timeout=120).read()
open(os.path.join(OUT, "data_challenges.json"), "wb").write(raw)
data = json.loads(raw.decode("utf-8"))
print("entries:", len(data))
for e in data:
    if e.get("id") == "wang-2024-pof-dg":
        print("KEYS:", sorted(e.keys()))
        print(json.dumps(e, indent=1, ensure_ascii=False)[:4000])
        break
# also search whole file for 'field' / 'wang2024'
txt = raw.decode("utf-8")
for kw in ("wang2024", "field.txt", "wake-field"):
    for m in re.finditer(re.escape(kw), txt):
        print(f"\nKW {kw} @ {m.start()}:", txt[max(0, m.start() - 150):m.start() + 250].replace("\n", " "))
        break
