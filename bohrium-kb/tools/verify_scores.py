#!/usr/bin/env python3
"""Re-verify current (settled) scores for all our round-3 attempts."""
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

CHALLENGES = {
    "construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97": "PPT",
    "reasoning-gate-gbsde-feynman-kac-e8970329": "GBSDE",
    "reasoning-gate-separable-covariance-1fe5635b": "permuton",
    "flowforge-open-model-selection-flow-v5-a9464888": "flowforge",
    "multi-sample-cnv-detection-from-binned-read-counts-15924b97": "CNV",
    "focused-imaging-and-resolution-characterisation-fr-e287fbca": "ultrasound",
    "mp-r-mp-r-a-uv-portal-a5be12b2": "UVportal",
    "mp-r-mp-r-ab-uv-split-coann-6924985d": "splitcoann",
    "solving-heterogeneous-agent-models-with-deepham-18a5adeb": "DeepHAM",
    "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd": "twisting",
}

for cid, label in CHALLENGES.items():
    try:
        r = requests.get(f"{BASE}/challenges/{cid}/attempts", headers=H, timeout=60)
        a = r.json()
        items = a if isinstance(a, list) else a.get("attempts", [])
        scored = [x for x in items if x.get("score") is not None]
        if scored:
            for x in scored[-3:]:
                print(f"{label:12s} attempt={x.get('id')} status={x.get('status')} "
                      f"score={x.get('score')} outcome={x.get('outcome')}")
        else:
            print(f"{label:12s} no scored attempts yet (total {len(items)})")
    except Exception as e:
        print(f"{label:12s} ERR {e}")
