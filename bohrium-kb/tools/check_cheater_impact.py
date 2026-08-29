#!/usr/bin/env python3
"""Live re-check friday's best attempts per round3 challenge (cheater impact)."""
import json
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
roster = json.load(open("_logs/roster_attempts.json", encoding="utf-8"))
cred = open(r"C:\Users\XKZ\.dsh\bohrium_credentials.txt", encoding="utf-8").read()
tok = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {tok}"}

best = {}
for slug, items in roster.items():
    cand = [a for a in items if a.get("authorId") == "friday" and (a.get("score") or 0) > 0]
    if cand:
        best[slug] = max(cand, key=lambda a: a.get("score") or 0)

print("=== friday best attempts (8/16 snapshot) live re-check ===")
for slug, a in sorted(best.items()):
    aid = a["id"]
    live = requests.get(f"https://play.bohrium.com/api/attempts/{aid}", headers=H, timeout=30).json()
    print(f"{slug[:34]:<36} aid={aid} snapshot={a.get('score')} LIVE={live.get('score')} status={live.get('status')}")

# also check all friday attempts with score>0 in snapshot for any -1000
print("\n=== any friday attempt now penalized (-1000)? ===")
penalized = []
for slug, items in roster.items():
    for a in items:
        if a.get("authorId") == "friday" and (a.get("score") or 0) > 0:
            live = requests.get(f"https://play.bohrium.com/api/attempts/{a['id']}", headers=H, timeout=30).json()
            if (live.get("score") or 0) < 0:
                penalized.append((slug, a["id"], a.get("score"), live.get("score")))
print(f"penalized attempts: {len(penalized)}")
for slug, aid, old, new in penalized[:20]:
    print(f"  {slug[:30]:<32} aid={aid} {old} -> {new}")
