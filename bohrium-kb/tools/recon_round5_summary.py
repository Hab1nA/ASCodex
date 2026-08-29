#!/usr/bin/env python3
"""Compact summary of recon data: challenge stats + attempts cache health."""
import json, os, collections

WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "round5-recon")

ch = json.load(open(os.path.join(OUT, "challenges_raw.json")))
print("n_challenges:", len(ch))
print("attempts-field distribution:", dict(collections.Counter(c.get("attempts") for c in ch)))
print("origin:", dict(collections.Counter(c.get("origin") for c in ch)))
print("disc:", dict(collections.Counter(c.get("disc") for c in ch)))
print("difficulty:", dict(collections.Counter(c.get("difficulty") for c in ch)))
print("roundSeq:", dict(collections.Counter(str(c.get("roundSeq")) for c in ch)))
print("roundId:", dict(collections.Counter(str(c.get("roundId")) for c in ch)))
print("status:", dict(collections.Counter(str(c.get("status")) for c in ch)))
print("stars:", dict(collections.Counter(c.get("stars") for c in ch)))
print("benchmarkId:", dict(collections.Counter(str(c.get("benchmarkId")) for c in ch)))

# non-zero attempts
nz = [c for c in ch if (c.get("attempts") or 0) > 0]
print("\nchallenges with attempts>0:", len(nz))
for c in sorted(nz, key=lambda x: -x["attempts"]):
    print(f"  att={c['attempts']:<4} diff={c.get('difficulty')} disc={c.get('disc'):<10} id={c['id']} | {c.get('title')}")

# attempts cache health
try:
    cache = json.load(open(os.path.join(OUT, "attempts_cache.json")))
    nonempty = {k: len(v) for k, v in cache.items() if v}
    print("\nattempts_cache: nonempty entries:", len(nonempty))
    for k, n in sorted(nonempty.items(), key=lambda kv: -kv[1])[:20]:
        print(f"  {n:<5} {k}")
    if not nonempty:
        print("  (all empty — either round is fresh or endpoint shape differs)")
except FileNotFoundError:
    print("\nattempts_cache.json missing")

# print a couple of full sample challenges that look 'real' (non-benchmark origin or non-mmlu)
samples = [c for c in ch if not str(c.get("id")).startswith("mmlu")]
print("\nnon-mmlu challenges:", len(samples))
for c in samples[:12]:
    print("  ", json.dumps({k: c.get(k) for k in ("id", "title", "difficulty", "disc", "attempts", "roundSeq", "origin", "tags", "hasContent", "hackathonSeasonId")}, ensure_ascii=False))
