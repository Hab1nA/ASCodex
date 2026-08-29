#!/usr/bin/env python3
"""Fresh concurrent fetch of our team's attempts across the round3 challenges.
Writes _logs/roster_attempts.json (per-challenge list of OUR attempts only)."""
import json
import os
import re
import sys
import threading
import time

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"
OUR_OPERATOR = "1179613"

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

results = {}
errors = []


def fetch(slug):
    out = []
    page = 1
    while True:
        try:
            r = requests.get(f"{BASE}/challenges/{slug}/attempts",
                             params={"per_page": 50, "page": page}, headers=H, timeout=60)
        except Exception as ex:
            errors.append((slug, page, str(ex)))
            break
        if r.status_code != 200:
            errors.append((slug, page, f"HTTP {r.status_code}"))
            break
        try:
            d = r.json() or {}
        except Exception:
            errors.append((slug, page, "bad json"))
            break
        items = d.get("attempts") or []
        total = d.get("total", 0)
        out.extend(items)
        if len(out) >= total or not items:
            break
        page += 1
        if page > 100:
            break
    ours = [a for a in out if str(a.get("operatorId") or "") == OUR_OPERATOR]
    results[slug] = {"total": len(out), "ours": ours}
    print(f"[done] {slug} total={len(out)} ours={len(ours)}", flush=True)


threads = [threading.Thread(target=fetch, args=(s,)) for s in SLUGS.values()]
for t in threads:
    t.start()
for t in threads:
    t.join(timeout=300)

print("errors:", errors, flush=True)
os.makedirs("_logs", exist_ok=True)
out = {k: v["ours"] for k, v in results.items()}
with open("_logs/roster_attempts.json", "w", encoding="utf-8") as f:
    json.dump(out, f, ensure_ascii=False, indent=1)
print("saved _logs/roster_attempts.json", flush=True)
