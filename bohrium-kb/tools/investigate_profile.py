#!/usr/bin/env python3
"""Investigate Bohrium profile: which agent identities are linked to the human
operator, and what the platform knows about each local credential file."""
import glob
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")

BASE = "https://play.bohrium.com/api"
CRED_DIR = os.path.expanduser("~/.dsh")

def load_token(path):
    txt = open(path, encoding="utf-8").read()
    m = re.search(r"api_token\s*=\s*(\S+)", txt)
    return m.group(1) if m else None

def me(tok):
    r = requests.get(f"{BASE}/auth/me", headers={"Authorization": f"Bearer {tok}"}, timeout=60)
    if r.status_code != 200:
        return {"_http": r.status_code, "_err": r.text[:200]}
    return r.json()

print("=" * 80)
print("STEP 1: identity of every local credential file (~/.dsh/*credentials.txt)")
print("=" * 80)
for path in sorted(glob.glob(os.path.join(CRED_DIR, "*credential*.txt"))):
    name = os.path.basename(path)
    tok = load_token(path)
    if not tok:
        print(f"{name}: no api_token found")
        continue
    d = me(tok)
    if "_http" in d:
        print(f"{name}: HTTP {d['_http']} {d['_err'][:100]}")
        continue
    rec = {k: d.get(k) for k in ("id", "name", "userType", "operatorId", "operatorName",
                                  "agentPersonaId", "personaName", "agentFramework", "email")}
    print(f"{name}: {json.dumps(rec, ensure_ascii=False)}")
