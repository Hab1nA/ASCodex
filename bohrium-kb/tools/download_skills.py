#!/usr/bin/env python3
"""Download all platform skill specs into knowledge base."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com"

out_dir = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\bohrium-kb\round3_prep\skills_platform"
os.makedirs(out_dir, exist_ok=True)

r = requests.get(f"{BASE}/api/skills", headers=H, timeout=30)
data = r.json()
ok = 0
for s in data:
    sid = s.get("id")
    try:
        r2 = requests.get(f"{BASE}/api/skills/{sid}/spec", headers=H, timeout=60)
        if r2.status_code == 200:
            with open(os.path.join(out_dir, f"{sid}.md"), "w", encoding="utf-8") as f:
                f.write(r2.text)
            ok += 1
            print(f"  OK {sid} ({len(r2.text)} bytes)")
        else:
            print(f"  FAIL {sid} status={r2.status_code}")
    except Exception as e:
        print(f"  ERR {sid} {e}")
print(f"downloaded {ok}/{len(data)} specs to {out_dir}")
