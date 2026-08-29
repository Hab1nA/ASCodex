#!/usr/bin/env python3
"""Analyze ground-state 03 judge signals from attempts_03.json (local snapshot)."""
import os, json, sys, collections
sys.stdout.reconfigure(encoding="utf-8")

d = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\bohrium-kb\round1_prep\research\attempts_03.json"
recs = json.load(open(d, encoding="utf-8"))
print("total:", len(recs))

def sc(r):
    s = r.get("scorecard") or {}
    return s

# ---- 1. Score distribution ----
dist = collections.Counter()
for r in recs:
    if r.get("status") != "scored": continue
    dist[round(r.get("score") or 0, 3)] += 1
print("\n=== scored distribution ===")
for k in sorted(dist, reverse=True):
    print(f"  {k:9.4f}: {dist[k]}")

# ---- 2. target tiers ----
targets = {"100": [], "96.875": [], "93.75": [], "90.625": [], "81.875": [], "78.75": [], "75": []}
for r in recs:
    if r.get("status") != "scored": continue
    scv = round(r.get("score") or 0, 3)
    for k, v in targets.items():
        if scv == float(k):
            s = sc(r)
            v.append({
                "id": r.get("id"), "author": r.get("author_name"), "harness": r.get("harness"),
                "model": r.get("modelTag"), "tc": r.get("traceCount"),
                "trace_score": s.get("trace_score"), "tq": s.get("trace_quality"),
                "harbor": s.get("harbor_reward"), "exec": s.get("executability"),
                "oc": s.get("output_coverage"), "pack": s.get("packaging"),
                "rf": s.get("result_fidelity"), "replay": s.get("harbor_replay_executed"),
                "created": (r.get("createdAt") or "")[:10],
                "method": r.get("method"),
            })

for k, v in targets.items():
    print(f"\n=== tier {k}: {len(v)} ===")
    for t in v:
        print(f"  id={t['id']} author='{t['author']}' harness='{t['harness']}' model='{t['model']}' "
              f"tc={t['tc']} trace={t['trace_score']} tq={t['tq']} harbor={t['harbor']} "
              f"exec={t['exec']} oc={t['oc']} pack={t['pack']} rf={t['rf']} replay={t['replay']}")

# ---- 3. Metadata aggregate for full-score vs 81.875 ----
def agg(tierkey):
    v = targets[tierkey]
    tcs = [t["tc"] for t in v if t["tc"] is not None]
    trs = [t["trace_score"] for t in v if t["trace_score"] is not None]
    harnesses = collections.Counter(t["harness"] for t in v)
    models = collections.Counter(t["model"] for t in v)
    return tcs, trs, harnesses, models

for tk in ("100", "81.875"):
    tcs, trs, hs, ms = agg(tk)
    print(f"\n=== AGG tier {tk} n={len(targets[tk])} ===")
    print(f"  traceCount: range {min(tcs)}-{max(tcs)} median={sorted(tcs)[len(tcs)//2]}")
    print(f"  trace_score: range {min(trs)}-{max(trs)}")
    print(f"  harness: {dict(hs)}")
    print(f"  model: {dict(ms)}")

# ---- 4. all attempts by tQ split (trace quality on/off) ----
print("\n=== tQ=0 attempts with score>0 (trace-neutral signals) ===")
for r in recs:
    s = sc(r)
    if r.get("score") and s.get("trace_quality") == 0.0 and r.get("score",0)>0:
        print(f"  id={r['id']} score={r['score']} harbor={s.get('harbor_reward')} model={r.get('modelTag')} author={r.get('author_name')}")
