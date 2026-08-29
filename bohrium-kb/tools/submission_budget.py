#!/usr/bin/env python3
"""Check submission counts per challenge and key attempt scores."""
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
    "gbsde": "reasoning-gate-gbsde-feynman-kac-e8970329",
    "permuton": "reasoning-gate-separable-covariance-1fe5635b",
    "flowforge": "flowforge-open-model-selection-flow-v5-a9464888",
    "cnv": "multi-sample-cnv-detection-from-binned-read-counts-15924b97",
    "ultrasound": "focused-imaging-and-resolution-characterisation-fr-e287fbca",
    "uv": "mp-r-mp-r-a-uv-portal-a5be12b2",
    "split": "mp-r-mp-r-ab-uv-split-coann-6924985d",
    "deepham": "solving-heterogeneous-agent-models-with-deepham-18a5adeb",
    "twist": "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd",
}
for label, cid in CH.items():
    r = requests.get(f"{BASE}/challenges/{cid}/attempts", headers=H,
                     params={"limit": 100}, timeout=60)
    items = r.json() if isinstance(r.json(), list) else r.json().get("attempts", [])
    mine = [x for x in items if (x.get("authorId") == "friday" or x.get("author_name") == "Friday")]
    best = max((x.get("score") or 0) for x in mine) if mine else 0
    print(f"{label:12s} my_attempts={len(mine):2d} best_score={best}")

# ultrasound 23900 current score
d = requests.get(f"{BASE}/attempts/23900", headers=H, timeout=60).json()
print("\nultrasound 23900:", d.get("status"), d.get("score"),
      json.dumps(d.get("resultsJson") or {}, ensure_ascii=False)[:200])
