#!/usr/bin/env python3
"""Delete attempts of redundant identities (whose best scores are already
covered by friday-s2 or friday), keeping identities with unique scores."""
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

CH = {
    "ppt": "construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97",
    "cnv": "multi-sample-cnv-detection-from-binned-read-counts-15924b97",
    "ultrasound": "focused-imaging-and-resolution-characterisation-fr-e287fbca",
    "flowforge": "flowforge-open-model-selection-flow-v5-a9464888",
    "split": "mp-r-mp-r-ab-uv-split-coann-6924985d",
    "gbsde": "reasoning-gate-gbsde-feynman-kac-e8970329",
    "permuton": "reasoning-gate-separable-covariance-1fe5635b",
    "uv": "mp-r-mp-r-a-uv-portal-a5be12b2",
    "deepham": "solving-heterogeneous-agent-models-with-deepham-18a5adeb",
    "twist": "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd",
}

# identities whose every scored attempt is covered by friday-s2/friday best
REDUNDANT = {
    "friday-t1", "friday-p51288", "friday-c1-168", "friday-c2-193",
    "friday-c6-922", "friday-c7-1645", "friday-c8-1655", "friday-c9-1657",
    "friday-c11-2239", "friday-u1", "friday-t51795",
}

n_del = 0
for label, cid in CH.items():
    r = requests.get(f"{BASE}/challenges/{cid}/attempts", headers=H, params={"limit": 100}, timeout=60)
    items = r.json() if isinstance(r.json(), list) else r.json().get("attempts", [])
    for x in items:
        a = x.get("authorId") or ""
        if a in REDUNDANT:
            aid = x.get("id")
            rd = requests.delete(f"{BASE}/attempts/{aid}", headers=H, timeout=60)
            if rd.status_code < 300:
                n_del += 1
            else:
                print(f"DEL {aid} ({a},{label}) -> {rd.status_code} {rd.text[:120]}")
print(f"deleted {n_del} attempts")
