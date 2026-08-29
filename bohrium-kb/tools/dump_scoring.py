#!/usr/bin/env python3
"""Dump full scoring details for worker attempts."""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")

cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

for aid in sys.argv[1:]:
    d = requests.get(f"{BASE}/attempts/{int(aid)}", headers=H, timeout=60).json()
    print("=" * 90)
    print(f"attempt {aid} status={d.get('status')} score={d.get('score')}")
    rj = d.get("resultsJson") or {}
    print("resultsJson:", json.dumps(rj, ensure_ascii=False, default=str)[:2500])
    sd = d.get("scoringDetails") or {}
    if sd:
        print("scoringDetails:", json.dumps(sd, ensure_ascii=False, default=str)[:2000])
