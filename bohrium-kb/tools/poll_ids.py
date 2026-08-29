#!/usr/bin/env python3
"""Poll specific attempt ids for status/score."""
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

ids = [int(x) for x in sys.argv[1:]]
for aid in ids:
    try:
        r = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60)
        d = r.json()
        rj = d.get("resultsJson") or {}
        src = (d.get("scoringDetails") or {}).get("source")
        print(f"aid={aid} status={d.get('status')} score={d.get('score')} "
              f"reward={d.get('harbor_reward')} trace={d.get('trace_score')} "
              f"author={d.get('authorId')} outcome={d.get('outcome')} "
              f"challenge={d.get('challengeId')} "
              f"src={src} err={rj.get('error_code') or rj.get('scored_by')}")
    except Exception as e:
        print(f"aid={aid} ERR {e}")
