#!/usr/bin/env python3
"""Full judge-signal mining for ground-state-shell-occupations problem (post-season)."""
import json, os, re, sys, time, collections
import requests
sys.stdout.reconfigure(encoding="utf-8")
sys.stderr.reconfigure(encoding="utf-8")

cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"
SLUG = "ground-state-shell-occupations-and-fbd-universal-3-2e27dc1a"

def get(path, **params):
    for _ in range(3):
        try:
            r = requests.get(BASE + path, headers=H, params=params, timeout=120)
            if r.status_code >= 400:
                print(f"GET {path} -> {r.status_code}: {r.text[:300]}", file=sys.stderr)
                time.sleep(1); continue
            return r.json()
        except Exception as e:
            print(f"GET {path} err: {e}", file=sys.stderr); time.sleep(1)
    return None

# Pull list window by paging (endpoint may ignore page; try anyway, also fetch detail for every id we can)
# First get the default window.
at = get(f"/challenges/{SLUG}/attempts", per_page=1000)
items = at.get("attempts") or at if isinstance(at, list) else at.get("attempts")
print("list window attempts:", len(items), "total:", at.get("total"))

# Try paging to collect more ids
all_ids = [a["id"] for a in items]
for page in range(2, 12):
    res = get(f"/challenges/{SLUG}/attempts", per_page=1000, page=page)
    if not res: break
    pg = res.get("attempts") if isinstance(res, dict) else res
    if not pg: break
    before = len(all_ids)
    all_ids += [a["id"] for a in pg]
    all_ids = list(dict.fromkeys(all_ids))
    print(f"  page {page}: +{len(all_ids)-before} unique -> total {len(all_ids)}")
    if len(all_ids) == before:
        break

print("\nDistinct attempt ids:", len(all_ids))

# Fetch full detail for each (capture scoringDetails, traceCount, scorecard, harness/modelTag)
recs = []
for i, aid in enumerate(all_ids):
    d = get(f"/attempts/{aid}")
    if not d: continue
    sc = d.get("scorecard") or {}
    rec = {
        "id": aid,
        "author_name": d.get("author_name"),
        "authorId": d.get("authorId"),
        "authorIsAgent": d.get("authorIsAgent"),
        "operatorId": d.get("operatorId"),
        "operatorName": d.get("operatorName"),
        "score": d.get("score"),
        "outcome": d.get("outcome"),
        "harness": d.get("harness"),
        "agentFramework": (d.get("agentFramework") or d.get("harness")),
        "modelTag": d.get("modelTag"),
        "method": d.get("method"),
        "createdAt": (d.get("createdAt") or "")[:19],
        "scorecard": sc,
        "scoringDetails": d.get("scoringDetails"),
        "resultsJson": d.get("resultsJson"),
        "detail": d.get("detail"),
        "execLog": d.get("execLog"),
        "traceCount": None,  # may be elsewhere
        "trace_score": sc.get("trace_score") or sc.get("traceScore"),
        "harbor_reward": sc.get("harbor_reward"),
        "reasoning_bonus": sc.get("reasoning_bonus"),
        "output_coverage": sc.get("output_coverage"),
        "executability": sc.get("executability"),
        "packaging": sc.get("packaging"),
        "result_fidelity": sc.get("result_fidelity"),
    }
    # raw top-level keys for max metadata capture
    rec["_topkeys"] = sorted(d.keys())
    recs.append(rec)
    if (i % 25) == 0:
        print(f"  fetched {i+1}/{len(all_ids)} aid={aid} score={rec['score']}")
    time.sleep(0.05)

out = os.path.join(os.path.dirname(__file__), "..", "round3_prep", "gs_r3_attempts_full.json")
os.makedirs(os.path.dirname(out), exist_ok=True)
json.dump(recs, open(out, "w", encoding="utf-8"), ensure_ascii=False, indent=2, default=str)
print("WROTE", out, "records:", len(recs))

# Distribution
dist = collections.Counter()
for r in recs:
    dist[round(r["score"] or 0, 3)] += 1
print("\n=== score distribution (from detail) ===")
for k in sorted(dist, reverse=True):
    print(f"  {k}: {dist[k]}")
