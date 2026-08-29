#!/usr/bin/env python3
"""Check worker-queue attempt scores."""
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
    23810: "permuton",
    23820: "PPT",
    23821: "GBSDE",
    23823: "flowforge",
    23824: "UVportal",
    23826: "splitcoann",
    23827: "CNV",
    23800: "UVportal-REST",
}

for aid, label in IDS.items():
    try:
        r = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60)
        d = r.json()
        rj = d.get("resultsJson") or {}
        src = (d.get("scoringDetails") or {}).get("source")
        err = rj.get("error_code") or rj.get("scored_by")
        print(f"{label:12s} {aid} status={d.get('status')} score={d.get('score')} "
              f"source={src} resultsJson_err={err} score%={rj.get('score_percent')}")
    except Exception as e:
        print(f"{label:12s} {aid} ERR {e}")
