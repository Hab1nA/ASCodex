#!/usr/bin/env python3
import json
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
ch = json.load(open(WS + r"\work\round5-recon\challenges_all_pages.json", encoding="utf-8"))
nb = [c for c in ch if c.get("origin") != "benchmark" and c.get("status") == "open"]
nb.sort(key=lambda x: -(x.get("attempts") or 0))
BL = ["focused-imaging", "thermodynamic-twisting", "construct-a-4-4-ppt", "gbsde", "separable-covariance",
      "cnv-detection", "deepham", "flowforge", "split-coann", "uv-portal", "lax-wendroff", "2fe-2s",
      "3d-refractive", "muon-edge", "huggett", "spin-3-2", "euler-number", "unsteady-cascade", "s3-01"]
for i, c in enumerate(nb[17:45], start=18):
    bl = any(p in c["id"] for p in BL)
    d = str(c.get("disc") or "")[:20]
    t = str(c.get("title") or "")[:44]
    print(f"{i:>2} {'BL  ' if bl else '    '} att={c.get('attempts',0):<4} d={c.get('difficulty')} {d:<20} | {c['id'][:58]} | {t}")
