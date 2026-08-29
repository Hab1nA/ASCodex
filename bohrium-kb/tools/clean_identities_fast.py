#!/usr/bin/env python3
"""Fast self-delete: list attempts by author and delete."""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")

BASE = "https://play.bohrium.com/api"

IDENTITY_FILES = {
    "friday-u1": "agent_u1_credentials.txt",
    "friday-t51795": "agent_t1_credentials.txt",
    "friday-u2": "friday-u2_credentials.txt",
    "friday-u3": "friday-u3_credentials.txt",
    "friday-r1": "friday-r1_credentials.txt",
    "friday-r2": "friday-r2_credentials.txt",
    "friday-r3": "friday-r3_credentials.txt",
    "friday-u4": "friday-u4-52367_credentials.txt",
    "friday-u5": "friday-u5-52903_credentials.txt",
    "friday-u6": "friday-u6-53704_credentials.txt",
    "friday-u7": "friday-u7-54212_credentials.txt",
    "friday-n1": "friday-n55379-n1_credentials.txt",
    "friday-n2": "friday-n55379-n2_credentials.txt",
    "friday-n3": "friday-n55379-n3_credentials.txt",
}

# map credential-file basename prefix -> expected authorId substring
NAME2AUTH = {}
for a, fn in IDENTITY_FILES.items():
    p = os.path.expanduser(f"~/.dsh/{fn}")
    if not os.path.exists(p):
        print(f"missing credential file: {fn}")
        continue
    txt = open(p, encoding="utf-8").read()
    m = re.search(r"api_token\s*=\s*(\S+)", txt)
    if not m:
        print(f"no token in {fn}")
        continue
    tok = m.group(1)
    H = {"Authorization": f"Bearer {tok}"}
    # resolve own id via /auth/me
    me = requests.get(f"{BASE}/auth/me", headers=H, timeout=60)
    uid = (me.json().get("id") or me.json().get("userId")) if me.status_code == 200 else None
    print(f"{a}: me -> {uid}")
    if not uid:
        continue
    r = requests.get(f"{BASE}/attempts", headers=H, params={"author": uid, "limit": 100}, timeout=60)
    try:
        items = r.json() if isinstance(r.json(), list) else r.json().get("attempts", [])
    except ValueError:
        items = []
    n = 0
    for x in items:
        aid = x.get("id")
        if not aid:
            continue
        rd = requests.delete(f"{BASE}/attempts/{aid}", headers=H, timeout=60)
        if rd.status_code < 300:
            n += 1
        else:
            print(f"  DEL {aid} -> {rd.status_code} {rd.text[:100]}")
    print(f"{a}: {len(items)} attempts, deleted {n}")
