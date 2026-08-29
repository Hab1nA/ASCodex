#!/usr/bin/env python3
"""Compare platform skill catalog vs local skills; dump missing ones."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
r = requests.get("https://play.bohrium.com/api/skills", headers=H, timeout=30)
data = r.json()
local = set(os.listdir(os.path.expanduser(r"~\.dsh\skills")))
print("== platform skills missing locally ==")
for s in data:
    sid = s.get("id")
    if sid not in local:
        desc = str(s.get("desc") or "")[:80]
        print(f"  {sid:28s} hasSpec={s.get('hasSpec')} domain={s.get('domain')} "
              f"forks={s.get('forks')} uses={s.get('uses')} | {desc}")
print()
print("== all platform skill ids ==")
print(", ".join(s.get("id") for s in data))
