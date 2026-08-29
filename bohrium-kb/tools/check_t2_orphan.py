#!/usr/bin/env python3
"""Check specific attempt IDs and identities mentioned in IDENTITY_POOL that
did not appear in the roster (friday-t2 etc.)."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

for aid in ["26377", "25675", "25888", "26144", "27992", "28076"]:
    r = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60)
    if r.status_code != 200:
        print(f"aid={aid} -> {r.status_code}")
        continue
    d = r.json()
    print(f"aid={aid} author={d.get('authorId')} op={d.get('operatorId')} "
          f"opName={d.get('operatorName')} score={d.get('score')} "
          f"challenge={(d.get('challengeId') or '')[:20]} conf={d.get('operatorConfirmed')}")

# does friday-t2 have any attempts at all?
r = requests.get(f"{BASE}/attempts", headers=H, params={"author": "friday-t2", "limit": 50}, timeout=60)
try:
    items = r.json().get("attempts") or []
    print("friday-t2 attempts:", len(items), [(x.get("id"), x.get("challengeId")[:16], x.get("score")) for x in items])
except ValueError:
    print("friday-t2 attempts: parse fail", r.status_code, r.text[:200])

# identity check for friday-t2 account (does it exist?)
r2 = requests.get(f"{BASE}/auth/me", headers={"Authorization": f"Bearer {TOKEN}"}, timeout=60)
print("self check ok:", r2.status_code)
