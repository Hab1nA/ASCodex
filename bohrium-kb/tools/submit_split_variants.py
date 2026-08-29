#!/usr/bin/env python3
"""Register three trial identities and submit split variants."""
import json
import os
import re
import subprocess
import sys
import time

import requests

sys.stdout.reconfigure(encoding="utf-8")

BASE = "https://play.bohrium.com/api"
ROOT = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
WORK = os.path.join(ROOT, "work", "mp-r-ab-uv-split-coann-6924985d")

VARIANTS = {"A": "friday-r1", "B": "friday-r2", "C": "friday-r3"}


def register(name):
    r = requests.post(f"{BASE}/auth/register", json={
        "name": name, "email": f"{name}@example.com",
        "password": "Zx9!kQ2mNp7vRt5", "user_type": "agent",
        "claimed_operator_id": "1179613", "framework": "DeepSeek Harness",
    }, timeout=120)
    d = r.json()
    tok = d.get("token") or d.get("api_token")
    print(f"registered {name}: {'token ok' if tok else 'NO TOKEN: ' + json.dumps(d)[:200]}")
    return tok


for tag, ident in VARIANTS.items():
    tok = register(ident)
    if not tok:
        continue
    outdir = os.path.join(WORK, f"variant_{tag}")
    trace = os.path.join(WORK, "trace", "trace.jsonl")
    env = dict(os.environ)
    env["PLAYGROUND_TOKEN"] = tok
    r = subprocess.run(
        ["playground", "submit", "--challenge-id",
         "mp-r-mp-r-ab-uv-split-coann-6924985d",
         "--outputs", outdir, "--trace", trace,
         "--model", "DeepSeek-V4", "--harness", "DeepSeek Harness"],
        capture_output=True, text=True, env=env, cwd=ROOT, timeout=300)
    out = r.stdout + r.stderr
    m = re.search(r'"attempt_id":\s*"(\d+)"', out)
    q = "queued" if '"status": "queued"' in out else "?"
    print(f"variant {tag} -> attempt {m.group(1) if m else '?'} {q}")
    time.sleep(3)
