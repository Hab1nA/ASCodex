#!/usr/bin/env python3
"""Show that identities whose attempts were deleted are STILL bound to the
operator (i.e., still listed on Profile's Registered Agent Accounts)."""
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
    return r.json() if r.status_code == 200 else {"_http": r.status_code}

# identities we cleaned (had local credentials) — check their CURRENT binding
check = ["friday-u3", "friday-u4-52367", "friday-u5-52903", "friday-s3-67618",
         "friday-n55379-n1", "friday-n55379-n2", "friday-n55379-n3"]
print("identities whose attempts were deleted -> current binding status:")
for path in sorted(glob.glob(os.path.join(CRED_DIR, "*credential*.txt"))):
    tok = load_token(path)
    if not tok:
        continue
    d = me(tok)
    if d.get("id") in check:
        print(f"  {d.get('id'):<22} operatorId={d.get('operatorId')} "
              f"operatorName={d.get('operatorName')} userType={d.get('userType')}")
