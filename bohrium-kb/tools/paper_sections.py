#!/usr/bin/env python3
"""Extract Data-preparation / validation / model sections from the paper text."""
import re

WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"
text = open(WS + r"\paper_2406_00695.txt", encoding="utf-8").read()
print("total chars:", len(text))

# locate section boundaries
for marker in ("II.1 Data preparation", "II Methodology", "II.2 Domain knowledge", "III Results", "III.2 The wake velocity deficit", "IV Validation", "V Conclusions"):
    i = text.find(marker)
    print(f"\n{'='*20} [{marker}] @ {i}")
    if i >= 0:
        print(text[i:i + 1800])
