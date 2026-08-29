#!/usr/bin/env python3
"""Poll latest attempts per scoring challenge for my identities."""
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
    "ppt": "construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97",
    "gbsde": "reasoning-gate-gbsde-feynman-kac-e8970329",
    "permuton": "reasoning-gate-separable-covariance-1fe5635b",
    "twist": "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd",
    "flowforge": "flowforge-open-model-selection-flow-v5-a9464888",
    "uv": "mp-r-mp-r-a-uv-portal-a5be12b2",
    "split": "mp-r-mp-r-ab-uv-split-coann-6924985d",
    "ultrasound": "focused-imaging-and-resolution-characterisation-fr-e287fbca",
    "cnv": "multi-sample-cnv-detection-from-binned-read-counts-15924b97",
    "deepham": "solving-heterogeneous-agent-models-with-deepham-18a5adeb",
}

for label, slug in slugs.items():
    try:
        r = requests.get(f"{BASE}/challenges/{slug}/attempts", params={"per_page": 30}, headers=H, timeout=60)
        items = (r.json() or {}).get("attempts") or []
        print("===", label, "total", (r.json() or {}).get("total"))
        for a in items:
            ident = str(a.get("authorId") or a.get("author_name") or "?").lower()
            if not any(k in ident for k in ("friday", "s2", "t2")):
                continue
            at = str(a.get("createdAt") or "")[:16]
            print(f"  aid={a.get('id')} status={a.get('status')} score={a.get('score')} "
                  f"reward={a.get('harbor_reward')} trace={a.get('trace_score')} who={ident} at={at}")
    except Exception as e:
        print("===", label, "ERR", e)
