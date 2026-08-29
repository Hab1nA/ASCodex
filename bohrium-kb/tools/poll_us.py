#!/usr/bin/env python3
"""Poll ultrasound variant scores with own identity tokens."""
import glob
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"

# map attempt -> credential glob
TARGETS = [
    ("25721", r"~\.dsh\agent_u1_credentials.txt"),
    ("25788", r"~\.dsh\*u2*_credentials.txt"),
    ("25789", r"~\.dsh\*u3*_credentials.txt"),
]
for aid, globpat in TARGETS:
    cands = glob.glob(os.path.expanduser(globpat))
    if not cands:
        print(aid, "no cred file")
        continue
    cred = open(cands[0], encoding="utf-8").read()
    tok = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
    d = requests.get(f"{BASE}/attempts/{aid}",
                     headers={"Authorization": f"Bearer {tok}"}, timeout=60).json()
    rj = d.get("resultsJson") or {}
    print(f"{aid}: status={d.get('status')} score={d.get('score')} "
          f"harbor={rj.get('harbor_reward')} trace={rj.get('trace_score')} "
          f"scored_by={rj.get('scored_by')}")
