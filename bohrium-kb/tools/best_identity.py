#!/usr/bin/env python3
"""Find the highest-scoring friday attempt per challenge and its identity."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}

slugs = {
    "T1 twist": "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd",
    "T2 permuton": "reasoning-gate-separable-covariance-1fe5635b",
    "T3 gbsde": "reasoning-gate-gbsde-feynman-kac-e8970329",
    "T4 ppt": "construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97",
    "T5 flowforge": "flowforge-open-model-selection-flow-v5-a9464888",
    "T6 uv": "mp-r-mp-r-a-uv-portal-a5be12b2",
    "T7 split": "mp-r-mp-r-ab-uv-split-coann-6924985d",
    "T8 deepham": "solving-heterogeneous-agent-models-with-deepham-18a5adeb",
    "T9 ultrasound": "focused-imaging-and-resolution-characterisation-fr-e287fbca",
    "T10 cnv": "multi-sample-cnv-detection-from-binned-read-counts-15924b97",
}

print(f"{'challenge':12s} {'score':>9s} {'harbor':>9s} {'trace':>7s} {'aid':>6s}  identity")
for label, slug in slugs.items():
    best = None
    page = 1
    fetched = 0
    total = 0
    while True:
        r = requests.get(
            f"https://play.bohrium.com/api/challenges/{slug}/attempts",
            params={"per_page": 20, "page": page}, headers=H, timeout=60)
        d = r.json() or {}
        items = d.get("attempts") or []
        total = d.get("total", 0)
        if not items:
            break
        fetched += len(items)
        for a in items:
            au = str(a.get("authorId") or "")
            if "friday" not in au.lower():
                continue
            sc = a.get("score")
            if sc is None:
                continue
            sd = a.get("scorecard") or {}
            harbor = sd.get("harbor_reward")
            ts = sd.get("trace_score")
            if best is None or (harbor is not None and harbor > (best[2] or -1)) or \
               (best[2] is None and harbor is not None) or \
               (harbor is None and best[2] is None and sc > best[1]):
                best = (a.get("id"), sc, harbor, ts, au, sd.get("source"))
        page += 1
        if fetched >= total or page > 30:
            break
    if best:
        aid, sc, harbor, ts, au, src = best
        print(f"{label:12s} {sc:9.4f} {str(harbor):>9s} {str(ts):>7s} {aid:>6d}  {au}  src={src}")
    else:
        print(f"{label:12s} (no friday attempts)")
