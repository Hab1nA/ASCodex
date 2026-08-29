#!/usr/bin/env python3
"""Targeted extraction: find body occurrences (skip TOC) of key terms."""
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"
text = open(WS + r"\paper_2406_00695.txt", encoding="utf-8").read()


def show(term, before=100, after=900, maxhits=3, skip_to=True):
    idxs = []
    start = 0
    while True:
        i = text.find(term, start)
        if i < 0:
            break
        idxs.append(i)
        start = i + 1
    print(f"\n##### '{term}': {len(idxs)} hits at {idxs[:8]}")
    shown = 0
    for i in idxs:
        if skip_to and i < 2500:  # skip TOC region
            continue
        print("  ...", text[max(0, i - before):i + after].replace("\n", " "), "...")
        shown += 1
        if shown >= maxhits:
            break


for term in ("Data preparation", "NREL", "5 MW", "5MW", "CFD", "computational fluid", "velocity deficit field", "125", "spanwise", "downstream", "mesh", "grid", "turbine model", "OpenFOAM", "LES", "RANS"):
    show(term)
