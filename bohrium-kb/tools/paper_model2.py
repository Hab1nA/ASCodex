#!/usr/bin/env python3
"""Extract body sections: use last occurrence of each heading."""
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"
text = open(WS + r"\paper_2406_00695.txt", encoding="utf-8").read()


def allidx(m):
    out, s = [], 0
    while True:
        i = text.find(m, s)
        if i < 0:
            return out
        out.append(i)
        s = i + 1


def body_section(start_m, end_m, maxchars=5200):
    si = allidx(start_m)
    ei = allidx(end_m)
    s = max([i for i in si if i > 5000], default=si[-1] if si else -1)
    e = min([i for i in ei if i > s], default=len(text))
    print(f"\n{'='*28} [{start_m}] body@{s} -> {e}")
    print(text[s:e][:maxchars])


body_section("Parameters for amplitude, mean and standard deviation", "The wake velocity deficit")
body_section("The wake velocity deficit", "IV Validation")
body_section("IV Validation", "V Conclusions")
body_section("B Expressions discovered by SR", "C The Bastankhah wake model")
