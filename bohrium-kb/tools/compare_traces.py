#!/usr/bin/env python3
"""Compare trace endpoints of a working (82.86) vs zeroed (0.0) attempt."""
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

for aid, label in [(23607, "permuton-first-82.86"), (23635, "permuton-zeroed"), (23745, "PPT-zeroed"), (23766, "ultrasound-100")]:
    tr = requests.get(f"{BASE}/attempts/{aid}/trace", headers=H, timeout=60)
    print(f"=== {label} (id {aid}): trace endpoint {tr.status_code}, {len(tr.text)} chars")
    try:
        steps = tr.json()
        print(f"    steps={len(steps) if isinstance(steps, list) else '?'}")
        if isinstance(steps, list) and steps:
            print("    first step keys:", list(steps[0].keys()))
            print("    step types:", [s.get("type") or s.get("step_type") for s in steps][:10])
    except Exception:
        print("    body:", tr.text[:300])
    print()
