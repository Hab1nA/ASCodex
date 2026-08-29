#!/usr/bin/env python3
"""Inspect challenge.json: status, scoring, image/execution hints, datasets."""
import json
import sys

p = sys.argv[1]
d = json.load(open(p, encoding="utf-8"))


def walk(obj, prefix="", depth=0):
    if depth > 2:
        return
    if isinstance(obj, dict):
        for k, v in obj.items():
            if isinstance(v, (dict, list)):
                print(f"{prefix}{k}: <{type(v).__name__} len={len(v)}>")
                walk(v, prefix + "  ", depth + 1)
            else:
                s = str(v)
                print(f"{prefix}{k}: {s[:120]}")
    elif isinstance(obj, list):
        for i, v in enumerate(obj[:6]):
            print(f"{prefix}[{i}]: {str(v)[:100]}")


print("=== TOP-LEVEL KEYS ===")
for k in d:
    v = d[k]
    t = type(v).__name__
    n = len(v) if isinstance(v, (list, dict, str)) else ""
    print(f"  {k}: {t} {n}")
print("\n=== scoring ===")
sc = d.get("scoring") or d.get("score") or {}
print(json.dumps(sc, ensure_ascii=False, indent=1)[:3000] if sc else "none")
print("\n=== image / execution / environment ===")
for key in ("image", "dockerImage", "baseImage", "execution", "runtime", "environment", "grader", "strategy", "grader_name", "score_range", "datasets", "models", "status", "roundEndAt"):
    if key in d:
        print(f"  {key}: {str(d[key])[:400]}")
print("\n=== full shallow dump (depth 2) ===")
walk(d)
