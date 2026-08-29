#!/usr/bin/env python3
"""Probe candidate static paths for field.txt; also read paper validation section."""
import urllib.request, os, json

UA = {"User-Agent": "Mozilla/5.0"}
OUT = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"

cands = [
    "/figures/wang-2024-pof-dg/field.txt",
    "/data/wang-2024-pof-dg/field.txt",
    "/datasets/wang2024-wake-field/field.txt",
    "/datasets/wang-2024-pof-dg/field.txt",
    "/files/wang-2024-pof-dg/field.txt",
    "/assets/wang2024-wake-field/field.txt",
    "/static/wang-2024-pof-dg/field.txt",
    "/figures/wang-2024-pof-dg/paper_fig1.jpg",
    "/data/challenges.json",
]
for p in cands:
    try:
        req = urllib.request.Request("https://play.bohrium.com" + p, headers=UA)
        r = urllib.request.urlopen(req, timeout=40)
        data = r.read()
        ct = r.headers.get("Content-Type", "")
        ishtml = "html" in ct.lower() or data[:14].lstrip().lower().startswith(b"<!doctype")
        print(f"{'HTML ' if ishtml else 'FILE'} {p} -> {len(data)}B {ct}")
        if not ishtml and len(data) > 100:
            open(os.path.join(OUT, "field_candidate_" + p.strip("/").replace("/", "_") + (".jpg" if p.endswith(".jpg") else ".txt")), "wb").write(data)
            print("    saved; head:", data[:100])
    except Exception as e:
        print(f"FAIL {p} -> {str(e)[:70]}")
