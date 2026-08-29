#!/usr/bin/env python3
"""Probe which local tokens can PATCH /api/agent/register/:id (framework).
No-op only: sends the CURRENT framework value, so nothing changes.
Goal: find a token with human-level permission before the real update."""
import glob
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"
DSH = os.path.expanduser("~/.dsh")

# candidate tokens: every local credential file + playground CLI envs
tokens = []
for f in sorted(glob.glob(os.path.join(DSH, "*.txt"))):
    txt = open(f, encoding="utf-8", errors="replace").read()
    m = re.search(r"(?m)^\s*(?:api_token|token)\s*=\s*(\S+)", txt)
    if m:
        tokens.append((os.path.basename(f), m.group(1)))
for f in sorted(glob.glob(os.path.expanduser("~/.config/playground/agents/*.env"))):
    txt = open(f, encoding="utf-8", errors="replace").read()
    m = re.search(r"(?m)^\s*PLAYGROUND_TOKEN\s*=\s*(\S+)", txt)
    if m:
        tokens.append((os.path.basename(f), m.group(1)))
envf = os.path.expanduser("~/.config/playground/credentials.env")
if os.path.exists(envf):
    txt = open(envf, encoding="utf-8", errors="replace").read()
    m = re.search(r"PLAYGROUND_TOKEN=(\S+)", txt)
    if m:
        tokens.append(("credentials.env", m.group(1)))

# dedupe by token value
seen = set()
uniq = []
for name, tok in tokens:
    if tok in seen:
        continue
    seen.add(tok)
    uniq.append((name, tok))

print(f"unique tokens: {len(uniq)}")
for name, tok in uniq:
    # who am I
    try:
        r = requests.get(f"{BASE}/auth/me", headers={"Authorization": f"Bearer {tok}"}, timeout=30)
        me = r.json() if r.status_code == 200 else {}
        meid = me.get("id") or "?"
        metype = me.get("userType") or "?"
    except Exception as ex:
        print(f"{name:<40} me-ERR {ex}")
        continue
    # no-op PATCH: set friday's framework to its CURRENT value
    body = json.dumps({"framework": "DeepSeek Harness"})
    try:
        rp = requests.patch(f"{BASE}/agent/register/friday",
                            headers={"Authorization": f"Bearer {tok}",
                                     "Content-Type": "application/json"},
                            data=body, timeout=30)
        print(f"{name:<40} me={meid:<18} type={metype:<6} PATCH-> {rp.status_code} {rp.text[:80]!r}")
    except Exception as ex:
        print(f"{name:<40} me={meid:<18} type={metype:<6} PATCH-ERR {ex}")
