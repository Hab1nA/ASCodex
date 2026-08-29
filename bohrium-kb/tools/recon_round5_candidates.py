#!/usr/bin/env python3
"""Rank non-benchmark challenges by attempt count; flag blacklist; save candidate table."""
import json, os, re, collections, urllib.request, time

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "round5-recon")

BLACKLIST_PREFIX = [
    "focused-imaging-and-resolution-characterisation", "thermodynamic-twisting-operator-diagnosis",
    "construct-a-4-4-ppt-quantum-channel", "reasoning-gate-gbsde-feynman-kac",
    "reasoning-gate-separable-covariance", "multi-sample-cnv-detection",
    "solving-heterogeneous-agent-models-with-deepham", "flowforge-open-model-selection-flow",
    "mp-r-mp-r-ab-uv-split-coann", "mp-r-mp-r-a-uv-portal", "lax-wendroff",
    "2fe-2s-sparse-ci-variational-energy-minimization", "3d-refractive-index-reconstruction",
    "muon-edge-reconstruction", "stationary-huggett-equilibrium", "spin-3-2-entanglement-power",
    "euler-number-approximation", "unsteady-cascade-transfer-functions",
    "s3-01",  # MgB2 Tc — archive/challenges/s3-01
]
OUR_OPERATOR = "1179613"

ch = json.load(open(os.path.join(OUT, "challenges_all_pages.json")))
nonbench = [c for c in ch if c.get("origin") != "benchmark"]
nonbench.sort(key=lambda x: -(x.get("attempts") or 0))

rows = []
for c in nonbench:
    cid = c["id"]
    bl = any(p in cid or p in str(c.get("title", "")) for p in BLACKLIST_PREFIX)
    rows.append({"id": cid, "title": c.get("title"), "attempts": c.get("attempts") or 0,
                 "difficulty": c.get("difficulty"), "disc": c.get("disc"), "roundSeq": c.get("roundSeq"),
                 "season": c.get("hackathonSeasonId"), "stars": c.get("stars"), "blacklisted": bl,
                 "status": c.get("status"), "reviewStatus": c.get("reviewStatus")})

json.dump(rows, open(os.path.join(OUT, "candidates_ranked.json"), "w", encoding="utf-8"), indent=1, ensure_ascii=False)
free = [r for r in rows if not r["blacklisted"]]
print("non-benchmark total:", len(rows), " | free (non-blacklisted):", len(free))
print()
for r in rows[:25]:
    tag = "BL  " if r["blacklisted"] else "    "
    print(f"{tag} att={r['attempts']:<4} diff={str(r['difficulty']):<3} disc={str(r['disc']):<20} r{r['roundSeq']} s{r['season']} | {r['id'][:58]} | {str(r['title'])[:52]}")

# check ours for the top 12 free candidates via /attempts
def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)

tok = load_token()
print("\n--- ours-check top free candidates ---")
for r in [x for x in rows if not x["blacklisted"]][:12]:
    try:
        req = urllib.request.Request(BASE + f"/challenges/{r['id']}/attempts", headers={"Authorization": "Bearer " + tok})
        with urllib.request.urlopen(req, timeout=90) as resp:
            d = json.loads(resp.read().decode())
        att = d if isinstance(d, list) else (d.get("items") or d.get("attempts") or d.get("data") or [])
        if isinstance(att, dict):  # paginated
            att = att.get("items", [])
        ours = sum(1 for a in att if a.get("operatorId") == OUR_OPERATOR or str(a.get("authorId", "")).startswith("friday") or str(a.get("authorName", "")).startswith("friday"))
        best = max((a.get("score") for a in att if isinstance(a.get("score"), (int, float))), default=None)
        print(f"  {r['id'][:58]} att={len(att)} ours={ours} best={best}")
        time.sleep(0.3)
    except Exception as e:
        print(f"  {r['id'][:58]} FAIL {e}")
