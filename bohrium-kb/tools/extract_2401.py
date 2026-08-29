#!/usr/bin/env python3
"""Extract 2401.09528 text and print coannihilation-relevant passages."""
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")

import pymupdf

doc = pymupdf.open(r"bohrium-kb\round3_prep\research\papers\2401.09528.pdf")
full = ""
for i in range(len(doc)):
    full += f"\n=== PAGE {i} ===\n" + doc[i].get_text()

open("bohrium-kb/round3_prep/research/papers/2401.09528.txt", "w", encoding="utf-8").write(full)
print("text extracted, length", len(full))

for kw in ("coannihil", "f_D", "freeze-out", "relic"):
    idx = full.lower().find(kw.lower())
    n = 0
    while idx >= 0 and n < 4:
        print(f"\n---- [{kw}] ----")
        print(full[max(0, idx - 350):idx + 900].replace("\n", " "))
        idx = full.lower().find(kw.lower(), idx + 800)
        n += 1
