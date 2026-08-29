#!/usr/bin/env python3
"""Pull GET /api/users (public user/agent directory), filter to our operator
(1179613 / 谢铠舟), and dump the authoritative agent-identity roster with
each account's declared agentFramework."""
import json
import os
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"
OUR_OPERATORS = {"1179613"}
OUR_OPERATOR_NAMES = {"谢铠舟"}

r = requests.get(f"{BASE}/users", timeout=120)
print("GET /api/users ->", r.status_code, f"({len(r.text)} bytes)")
if r.status_code != 200:
    print(r.text[:500])
    sys.exit(1)

users = r.json()
print(f"total users in directory: {len(users)}")

ours = []
for uid, u in users.items():
    op = str(u.get("operatorId") or "")
    opname = u.get("operatorName") or ""
    if op in OUR_OPERATORS or opname in OUR_OPERATOR_NAMES:
        ours.append(u)

print(f"users under our operator: {len(ours)}\n")
for u in sorted(ours, key=lambda x: str(x.get("id"))):
    print(json.dumps({
        "id": u.get("id"),
        "name": u.get("name"),
        "userType": u.get("userType"),
        "agentFramework": u.get("agentFramework"),
        "operatorId": u.get("operatorId"),
        "operatorName": u.get("operatorName"),
        "operatorConfirmed": u.get("operatorConfirmed"),
        "persona": (u.get("persona") or {}).get("name") if isinstance(u.get("persona"), dict) else None,
    }, ensure_ascii=False))

os.makedirs("_logs", exist_ok=True)
with open("_logs/users_ours.json", "w", encoding="utf-8") as f:
    json.dump(ours, f, ensure_ascii=False, indent=1)
print("\nsaved _logs/users_ours.json")
