#!/usr/bin/env python3
"""Full attempt list (id/status/score) per challenge for a given identity."""
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

slugs = {
    "T1_twist": "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd",
    "T2_permuton": "reasoning-gate-separable-covariance-1fe5635b",
    "T3_gbsde": "reasoning-gate-gbsde-feynman-kac-e8970329",
    "T4_ppt": "construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97",
    "T5_flowforge": "flowforge-open-model-selection-flow-v5-a9464888",
    "T6_uv": "mp-r-mp-r-a-uv-portal-a5be12b2",
    "T7_split": "mp-r-mp-r-ab-uv-split-coann-6924985d",
    "T8_deepham": "solving-heterogeneous-agent-models-with-deepham-18a5adeb",
    "T9_ultrasound": "focused-imaging-and-resolution-characterisation-fr-e287fbca",
    "T10_cnv": "multi-sample-cnv-detection-from-binned-read-counts-15924b97",
}

labels = sys.argv[1:] or list(slugs.keys())
ident = "friday-s2-24714"

for label in labels:
    slug = slugs[label]
    print("===", label)
    page = 1
    fetched = 0
    total_all = 0
    while True:
        r = requests.get(f"{BASE}/challenges/{slug}/attempts",
                         params={"per_page": 20, "page": page}, headers=H, timeout=60)
        d = r.json() or {}
        items = d.get("attempts") or []
        total_all = d.get("total", 0)
        if not items:
            break
        fetched += len(items)
        for a in items:
            if str(a.get("authorId") or "") != ident:
                continue
            print(f"  aid={a.get('id')} status={a.get('status')} score={a.get('score')} "
                  f"outcome={a.get('outcome')} at={(a.get('createdAt') or '')[:16]}")
        page += 1
        if fetched >= total_all or page > 30:
            break
