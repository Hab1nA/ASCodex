#!/usr/bin/env python3
"""Pure-harbor distribution + 160-grid check + our attempt 判词 via API."""
import os, json, sys, collections, re
import requests
sys.stdout.reconfigure(encoding="utf-8")

# --- local pure-harbor distribution ---
recs = json.load(open(r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\bohrium-kb\round1_prep\research\attempts_03.json", encoding="utf-8"))
print("=== scored distribution by score ===")
dist = collections.Counter()
for r in recs:
    if r.get("status") != "scored": continue
    dist[round(r.get("score") or 0,3)] += 1
for k in sorted(dist, reverse=True):
    mark = "  <-- OUR TIER" if abs(k-81.875)<0.001 else ""
    print(f"  {k:9.4f}: {dist[k]}{mark}")

print("\n=== scored by PURE harbor_reward (content only) ===")
hdist = collections.Counter()
for r in recs:
    if r.get("status") != "scored": continue
    s = r.get("scorecard") or {}
    h = s.get("harbor_reward")
    if h is None: continue
    hdist[round(h,5)] += 1
for k in sorted(hdist, reverse=True):
    print(f"  {k:.5f} (= {k*160:.1f}/160, {k*16:.1f}/16): {hdist[k]}")

print("\n=== 82-87.5 band presence check (score between 81.9 and 87.5) ===")
band = [r for r in recs if r.get("status")=="scored" and 81.9 <= (r.get('score') or 0) < 87.5]
print("attempts in [81.9,87.5):", len(band))
for r in band:
    print("  ", r["id"], r["score"], r.get("author_name"))

# --- API: re-fetch our attempt + one full scorer for execLog/scoringDetails ---
def get_token(name):
    p = os.path.expanduser(fr"~\.dsh\{name}.txt")
    t = open(p, encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", t).group(1)

TOKEN = get_token("bohrium_credentials")
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

for aid in ["28923", "28927"]:
    d = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60).json()
    print(f"\n=== OUR attempt {aid} ===  score={d.get('score')}")
    print("  scoringDetails:", json.dumps(d.get("scoringDetails"), ensure_ascii=False))
    print("  resultsJson:", json.dumps(d.get("resultsJson"), ensure_ascii=False)[:500])
    print("  execLog:", json.dumps(d.get("execLog"), ensure_ascii=False)[:800])
