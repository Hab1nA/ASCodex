#!/usr/bin/env python3
"""Enumerate ALL attempts of our team (operator 1179613) across the 10 round-3
challenges. Compute per-challenge top scores and per-identity summaries.

Output:
  - identity roster (distinct authorIds under operator 1179613)
  - per challenge: best score per identity, and the team's #1 score + holder
  - per identity: best scores, whether it holds any challenge top score
  - full data dumped to _logs/roster_attempts.json for later steps
"""
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

SLUGS = {
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

OUR_OPERATOR = "1179613"

def fetch_all(slug):
    """Fetch every attempt of a challenge, handling pagination (page cap 20)."""
    out = []
    page = 1
    while True:
        r = requests.get(f"{BASE}/challenges/{slug}/attempts",
                         params={"per_page": 20, "page": page}, headers=H, timeout=90)
        d = r.json() or {}
        items = d.get("attempts") or []
        total = d.get("total", 0)
        out.extend(items)
        if len(out) >= total or not items:
            break
        page += 1
        if page > 200:
            break
    return out

def score_key(a):
    sd = a.get("scorecard") or {}
    harbor = sd.get("harbor_reward")
    if harbor is None:
        harbor = sd.get("harbor_score")
    return (a.get("score") or 0.0, harbor or 0.0)

all_data = {}
for label, slug in SLUGS.items():
    items = fetch_all(slug)
    ours = [a for a in items if str(a.get("operatorId") or "") == OUR_OPERATOR]
    all_data[label] = ours
    print(f"== {label}: total={len(items)} ours={len(ours)}")

os.makedirs(os.path.join(os.path.dirname(__file__), "..", "..", "_logs"), exist_ok=True)
with open(os.path.join(os.path.dirname(__file__), "..", "..", "_logs", "roster_attempts.json"),
          "w", encoding="utf-8") as f:
    json.dump(all_data, f, ensure_ascii=False, indent=1)
print("saved _logs/roster_attempts.json")
