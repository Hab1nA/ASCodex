#!/usr/bin/env python3
"""Find the currently-open round window and list open, non-benchmark challenges in it."""
import json, os, collections, datetime

WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "round5-recon")
ch = json.load(open(os.path.join(OUT, "challenges_all_pages.json"), encoding="utf-8"))

NOW = datetime.datetime(2026, 8, 18, 4, 5, tzinfo=datetime.timezone.utc)

def parse(s):
    if not s:
        return None
    return datetime.datetime.fromisoformat(s.replace("Z", "+00:00"))

win = collections.Counter()
for c in ch:
    if c.get("origin") == "benchmark":
        continue
    win[(str(c.get("roundStartAt")), str(c.get("roundEndAt")), str(c.get("status")))] += 1

print("window / status histogram (non-benchmark):")
for (s, e, st), n in sorted(win.items()):
    sdt = parse(s); edt = parse(e)
    live = "  <-- LIVE NOW" if (sdt and edt and sdt <= NOW <= edt) else ("  (future)" if sdt and sdt > NOW else "  (past)")
    print(f"  {n:>4}  {s} .. {e}  status={st}{live}")

open_now = [c for c in ch if c.get("origin") != "benchmark" and c.get("status") == "open"]
print("\nstatus=open non-benchmark:", len(open_now))
open_now.sort(key=lambda x: -(x.get("attempts") or 0))
BL = ["focused-imaging", "thermodynamic-twisting", "construct-a-4-4-ppt", "gbsde", "separable-covariance",
      "cnv-detection", "deepham", "flowforge", "split-coann", "uv-portal", "lax-wendroff", "2fe-2s",
      "3d-refractive", "muon-edge", "huggett", "spin-3-2", "euler-number", "unsteady-cascade", "s3-01"]
for c in open_now[:40]:
    bl = any(p in c["id"] for p in BL)
    tag = "BL  " if bl else "    "
    print(f"{tag} att={c.get('attempts',0):<4} diff={str(c.get('difficulty')):<3} disc={str(c.get('disc')):<20} {c['id'][:60]} | {str(c.get('title'))[:55]}")
