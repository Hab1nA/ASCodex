#!/usr/bin/env python3
"""Parse anonymous competition leaderboard data.json."""
import json
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")

d = requests.get(
    "http://nwjs1473070.bohrium.tech:50001/competition-leaderboard/data.json", timeout=60
).json()

chall = {c["code"]: c["title"][:28] for c in d["challenges"]}
print("generated", d.get("generatedAt"), "| participants", d.get("competition", {}).get("participantCount"))

# Structure discovery
rows = d.get("standings") or []
if not rows:
    print("keys:", sorted(d.keys()))
    sys.exit(0)

print("challenges:", json.dumps(d.get("challenges"), ensure_ascii=False)[:1200])
print("taskMaxima:", json.dumps(d.get("taskMaxima"), ensure_ascii=False)[:500])
print("---")
print("standings count:", len(rows))
for r in rows:
    name = str(r.get("participant") or "?")
    print(f"{r.get('rank')} | {name} | score={r.get('score')} | {json.dumps(r.get('scores'), ensure_ascii=False)}")



