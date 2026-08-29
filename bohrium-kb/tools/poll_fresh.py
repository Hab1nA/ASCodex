#!/usr/bin/env python3
"""Poll fresh friday attempts (after a given time) per challenge."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}

slugs = {
    "twist": "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd",
    "permuton": "reasoning-gate-separable-covariance-1fe5635b",
    "deepham": "solving-heterogeneous-agent-models-with-deepham-18a5adeb",
    "ultrasound": "focused-imaging-and-resolution-characterisation-fr-e287fbca",
    "uv": "mp-r-mp-r-a-uv-portal-a5be12b2",
    "flowforge": "flowforge-open-model-selection-flow-v5-a9464888",
    "gbsde": "reasoning-gate-gbsde-feynman-kac-e8970329",
    "split": "mp-r-mp-r-ab-uv-split-coann-6924985d",
    "cnv": "multi-sample-cnv-detection-from-binned-read-counts-15924b97",
    "ppt": "construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97",
}

since = sys.argv[1] if len(sys.argv) > 1 else "2026-08-15T03:00"
labels = sys.argv[2:] or list(slugs.keys())
found = 0
for label in labels:
    r = requests.get(
        f"https://play.bohrium.com/api/challenges/{slugs[label]}/attempts",
        params={"per_page": 20, "page": 1}, headers=H, timeout=60)
    items = (r.json() or {}).get("attempts") or []
    for a in items:
        if str(a.get("createdAt") or "") <= since:
            continue
        au = str(a.get("authorId") or "")
        if "friday" not in au.lower():
            continue
        found += 1
        print(f"{label:11s} aid={a.get('id')} author={au} status={a.get('status')} "
              f"score={a.get('score')} at={(a.get('createdAt') or '')[11:16]}")
if not found:
    print("(no fresh friday attempts)")
