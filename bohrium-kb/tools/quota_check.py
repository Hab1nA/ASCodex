#!/usr/bin/env python3
"""Count submissions per challenge for a given identity."""
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

ident = sys.argv[1] if len(sys.argv) > 1 else "friday-s2-24714"

for label, slug in slugs.items():
    counts = {"draft": 0, "scoring": 0, "scored": 0, "submitted": 0, "failed": 0, "total": 0}
    best = None
    page = 1
    fetched = 0
    total_all = None
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
            st = a.get("status") or "?"
            counts["total"] += 1
            counts[st] = counts.get(st, 0) + 1
            if st == "scored" and a.get("score") is not None:
                if best is None or a["score"] > best[0]:
                    best = (a["score"], a.get("id"), a.get("outcome"))
        page += 1
        if fetched >= (total_all or 0) or page > 30:
            break
    b = f" best={best}" if best else ""
    print(f"{label:14s} n={counts['total']:2d} draft={counts['draft']} scoring={counts['scoring']} "
          f"scored={counts['scored']} submitted={counts['submitted']} failed={counts['failed']}{b}")
