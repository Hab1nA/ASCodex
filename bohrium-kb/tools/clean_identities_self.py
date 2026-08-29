#!/usr/bin/env python3
"""Delete attempts using each identity's own token (self-delete)."""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")

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

# identity -> credential file (identities with tokens on disk)
IDENTITY_FILES = {
    "friday-u1": "agent_u1_credentials.txt",
    "friday-t51795": "agent_t1_credentials.txt",
    "friday-u2": "friday-u2_credentials.txt",
    "friday-u3": "friday-u3_credentials.txt",
    "friday-r1": "friday-r1_credentials.txt",
    "friday-r2": "friday-r2_credentials.txt",
    "friday-r3": "friday-r3_credentials.txt",
    "friday-u4": "friday-u4-52367_credentials.txt",
    "friday-u5": "friday-u5-52903_credentials.txt",
    "friday-u6": "friday-u6-53704_credentials.txt",
    "friday-u7": "friday-u7-54212_credentials.txt",
    "friday-n1": "friday-n55379-n1_credentials.txt",
    "friday-n2": "friday-n55379-n2_credentials.txt",
    "friday-n3": "friday-n55379-n3_credentials.txt",
}

REDUNDANT = {"friday-u1", "friday-t51795", "friday-u2", "friday-u3",
             "friday-u4", "friday-u5", "friday-u6", "friday-u7",
             "friday-n1", "friday-n2", "friday-n3"}

tokens = {}
for a, fn in IDENTITY_FILES.items():
    p = os.path.expanduser(f"~/.dsh/{fn}")
    if os.path.exists(p):
        txt = open(p, encoding="utf-8").read()
        m = re.search(r"api_token\s*=\s*(\S+)", txt)
        if m:
            tokens[a] = m.group(1)

n_del = 0
for label, cid in CH.items():
    # enumerate with a fresh identity token per identity
    for a, tok in tokens.items():
        if a not in REDUNDANT:
            continue
        H = {"Authorization": f"Bearer {tok}"}
        r = requests.get(f"{BASE}/challenges/{cid}/attempts", headers=H,
                         params={"limit": 100, "author": a}, timeout=60)
        try:
            items = r.json() if isinstance(r.json(), list) else r.json().get("attempts", [])
        except ValueError:
            items = []
        for x in items:
            if x.get("authorId") == a:
                aid = x.get("id")
                rd = requests.delete(f"{BASE}/attempts/{aid}", headers=H, timeout=60)
                if rd.status_code < 300:
                    n_del += 1
                else:
                    print(f"DEL {aid} ({a},{label}) -> {rd.status_code} {rd.text[:100]}")
print(f"deleted {n_del} attempts (identities with on-disk tokens)")
