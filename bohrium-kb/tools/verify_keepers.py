#!/usr/bin/env python3
"""Verify n55379-n3 binding; confirm keeper attempts still exist live."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"

def me(tok):
    r = requests.get(f"{BASE}/auth/me", headers={"Authorization": f"Bearer {tok}"}, timeout=60)
    return r.json() if r.status_code == 200 else {"_http": r.status_code}

for fn in ["friday-n55379-n3_credentials.txt"]:
    p = os.path.expanduser(f"~/.dsh/{fn}")
    if not os.path.exists(p):
        print(fn, "missing")
        continue
    txt = open(p, encoding="utf-8").read()
    m = re.search(r"api_token\s*=\s*(\S+)", txt)
    d = me(m.group(1))
    print(fn, "->", {k: d.get(k) for k in ("id", "name", "userType", "operatorId", "operatorName")})

# keeper attempts still live?
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
for aid in ["28013", "26103", "28076", "28045", "26873", "28047", "28025", "23607", "23701", "23821", "26144", "27992", "28064"]:
    r = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60)
    d = r.json() if r.status_code == 200 else {}
    print(f"aid={aid} -> {r.status_code} author={d.get('authorId')} score={d.get('score')}")
