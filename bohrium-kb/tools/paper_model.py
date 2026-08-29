#!/usr/bin/env python3
"""Extract the paper's discovered wake expressions (Sec III, App B) and Validation (Sec IV)."""
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"
text = open(WS + r"\paper_2406_00695.txt", encoding="utf-8").read()


def section(start_marker, end_markers, maxchars=5000):
    i = text.find(start_marker)
    if i < 0:
        print(f"!! marker not found: {start_marker}")
        return
    j = len(text)
    for em in end_markers:
        k = text.find(em, i + 10)
        if k > i:
            j = min(j, k)
    seg = text[i:j][:maxchars]
    print(f"\n{'='*30} [{start_marker}] ({len(seg)} chars)")
    print(seg)


# Results section III (amplitude/mean/std parameters)
section("III.1 Parameters for amplitude", ["III.2 The wake velocity deficit"], 4500)
section("III.2 The wake velocity deficit", ["IV Validation"], 4500)
section("IV Validation", ["V Conclusions"], 3500)
section("B Expressions discovered by SR", ["C The Bastankhah wake model"], 6000)
