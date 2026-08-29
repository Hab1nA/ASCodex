#!/usr/bin/env python3
"""Verify settled scores of specific attempt ids (authoritative detail query)."""
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

IDS = {
    "permuton": 23635,
    "splitcoann": 23685,
    "GBSDE": 23681,
    "PPT": 23745,
    "ultrasound": 23766,
    "flowforge": 23730,
    "UVportal": 23740,
}

for label, aid in IDS.items():
    try:
        r = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60)
        if r.status_code != 200:
            print(f"{label:12s} {aid} HTTP {r.status_code}")
            continue
        d = r.json()
        if isinstance(d, dict):
            st = d.get("status")
            sc = d.get("score")
            oc = d.get("outcome")
            hs = d.get("harbor_score")
            scd = d.get("scoringDetails") or d.get("scoring_details")
            print(f"{label:12s} id={aid} status={st} score={sc} outcome={oc} harbor={hs}")
            if scd:
                s = json.dumps(scd, ensure_ascii=False)
                print("    details:", s[:400])
        else:
            print(label, aid, type(d).__name__, str(d)[:200])
    except Exception as e:
        print(f"{label:12s} {aid} ERR {e}")
