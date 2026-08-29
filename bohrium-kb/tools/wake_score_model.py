#!/usr/bin/env python3
"""Regress score vs (r_squared, sections_fitted) over all fetched attempts."""
import json, os, collections

WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "wake-dg-fit")
items = json.load(open(os.path.join(OUT, "attempts_all.json"), encoding="utf-8"))

rows = []
for a in items:
    rj = a.get("resultsJson") or {}
    r2 = rj.get("r_squared")
    sf = rj.get("sections_fitted")
    rows.append((a.get("score"), r2, sf, a.get("status"), a.get("traceCount"), a.get("id")))

print(f"{'score':>6} {'r2':>9} {'sections':>8} {'status':<14} {'traces':>6} id")
for s, r2, sf, st, tc, aid in sorted(rows, key=lambda r: (-(r[0] or 0), r[5])):
    print(f"{str(s):>6} {str(r2):>9} {str(sf):>8} {str(st):<14} {str(tc):>6} {aid}")

print("\nstatus x score:")
c = collections.Counter()
for s, r2, sf, st, tc, aid in rows:
    c[(st, s)] += 1
for k in sorted(c, key=lambda x: (str(x[0]), -(x[1] or 0))):
    print(" ", k, c[k])
