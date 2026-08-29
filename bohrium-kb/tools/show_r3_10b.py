#!/usr/bin/env python3
"""Inspect traceHead / execLog / detail / leaderboard of top attempts from judge_r3_10_data.json."""
import json, sys
sys.stdout.reconfigure(encoding="utf-8")
P = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\bohrium-kb\round3_prep\judge_r3_10_data.json"
d = json.load(open(P, encoding="utf-8"))

print("=== LEADERBOARD ===")
lb = d.get("leaderboard")
if lb:
    for x in lb[:15]:
        print(x)
else:
    print("none")

print("\n=== TOP attempts traceHead (first 200 chars) / execLog / detail ===")
for t in d["top"]:
    th = t.get("traceHead") or ""
    el = t.get("execLog")
    det = str(t.get("detail"))[:120]
    print("\naid=%s %s score=%s" % (t.get("id"), t.get("author_name"), t.get("score")))
    print("  traceHead:", th[:200].replace("\n", " / "))
    print("  execLog(type=%s):" % type(el).__name__, (str(el)[:150] if el else "None"))
    print("  detail:", det)
