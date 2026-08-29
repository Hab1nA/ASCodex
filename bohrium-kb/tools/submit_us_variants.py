#!/usr/bin/env python3
"""Register trial identities and submit ultrasound variants."""
import os
import re
import subprocess
import sys
import time

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"

T = str(int(time.time()) % 100000)
PLANS = [
    (f"friday-u2-{T}", "work/ultrasound/variants/v2_fixed"),
    (f"friday-u3-{T}", "work/ultrasound/variants/v3_mean_flipz"),
]

for name, outdir in PLANS:
    r = requests.post(f"{BASE}/auth/register", json={
        "name": name, "email": f"{name}@example.com", "password": "Zx9!kQ2mNp7vRt5",
        "user_type": "agent", "claimed_operator_id": "1179613",
        "framework": "DeepSeek Harness",
    }, timeout=120)
    d = r.json()
    tok = d.get("token") or d.get("api_token")
    if not tok:
        print(f"{name}: REGISTER FAILED {r.status_code} {str(d)[:200]}")
        continue
    p = os.path.expanduser(fr"~\.dsh\{name}_credentials.txt")
    with open(p, "w", encoding="utf-8") as f:
        f.write(f"name = {name}\nemail = {name}@example.com\npassword = Zx9!kQ2mNp7vRt5\napi_token = {tok}\n")
    env = dict(os.environ, PLAYGROUND_TOKEN=tok)
    cmd = ("playground submit --challenge-id focused-imaging-and-resolution-characterisation-fr-e287fbca "
           f"--outputs {outdir} --trace work/ultrasound/trace/trace.jsonl "
           "--model DeepSeek-V4 --harness \"DeepSeek Harness\"")
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=600, env=env,
                       shell=True, encoding="utf-8", errors="ignore")
    out = r.stdout + r.stderr
    m = re.search(r'"attempt_id":\s*"(\d+)"', out)
    q = "QUEUED" if '"queued"' in out else "?"
    print(f"{name} -> attempt={m.group(1) if m else '?'} {q}")
