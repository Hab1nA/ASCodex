#!/usr/bin/env python3
"""List status=open non-benchmark challenges (currently submittable), ranked by attempts."""
import json, os, collections

WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "round5-recon")
ch = json.load(open(os.path.join(OUT, "challenges_all_pages.json"), encoding="utf-8"))

nb = [c for c in ch if c.get("origin") != "benchmark"]
print("status histogram (non-benchmark):", dict(collections.Counter(str(c.get("status")) for c in nb)))

BL = ["focused-imaging", "thermodynamic-twisting", "construct-a-4-4-ppt", "gbsde", "separable-covariance",
      "cnv-detection", "deepham", "flowforge", "split-coann", "uv-portal", "lax-wendroff", "2fe-2s",
      "3d-refractive", "muon-edge", "huggett", "spin-3-2", "euler-number", "unsteady-cascade", "s3-01"]

open_now = [c for c in nb if c.get("status") == "open"]
open_now.sort(key=lambda x: -(x.get("attempts") or 0))
print("status=open non-benchmark:", len(open_now))
for c in open_now[:45]:
    bl = any(p in c["id"] for p in BL)
    tag = "BL  " if bl else "    "
    print(f"{tag} att={c.get('attempts',0):<4} diff={str(c.get('difficulty')):<3} disc={str(c.get('disc')):<22} win={str(c.get('roundStartAt'))[:10]}..{str(c.get('roundEndAt'))[:10]} | {c['id'][:56]} | {str(c.get('title'))[:50]}")
