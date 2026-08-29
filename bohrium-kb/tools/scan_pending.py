#!/usr/bin/env python3
"""Scan all challenges for our pending/unscored attempts."""
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
    "gbsde": "reasoning-gate-gbsde-feynman-kac-e8970329",
    "ppt": "construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97",
    "flowforge": "flowforge-open-model-selection-flow-v5-a9464888",
    "uv": "mp-r-mp-r-a-uv-portal-a5be12b2",
    "split": "mp-r-mp-r-ab-uv-split-coann-6924985d",
    "deepham": "solving-heterogeneous-agent-models-with-deepham-18a5adeb",
    "ultrasound": "focused-imaging-and-resolution-characterisation-fr-e287fbca",
    "cnv": "multi-sample-cnv-detection-from-binned-read-counts-15924b97",
}

found = 0
for label, slug in slugs.items():
    r = requests.get(
        f"https://play.bohrium.com/api/challenges/{slug}/attempts",
        params={"per_page": 20, "page": 1}, headers=H, timeout=60)
    items = (r.json() or {}).get("attempts") or []
    for a in items:
        au = str(a.get("authorId") or "")
        if "friday" not in au.lower():
            continue
        st = a.get("status")
        sc = a.get("score")
        at = a.get("createdAt") or ""
        is_pending = (st == "scoring") or (
            st == "scored" and (sc in (0, 0.0)) and at > "2026-08-15T10:00")
        if is_pending:
            found += 1
            sd = a.get("scoringDetails") or {}
            rj = a.get("resultsJson") or {}
            print(f"{label:11s} aid={a.get('id')} author={au:16s} status={st} "
                  f"score={sc} at={at[11:19]} src={sd.get('source')}")
if not found:
    print("(no pending friday attempts)")
