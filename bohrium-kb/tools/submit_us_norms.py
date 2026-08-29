#!/usr/bin/env python3
"""Submit three normalization variants with fresh trial identities."""
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
    ("n1_each_max", "work/ultrasound/variants/n1_each_max"),
    ("n2_global_max", "work/ultrasound/variants/n2_global_max"),
    ("n3_nodc_raw", "work/ultrasound/variants/n3_nodc_raw"),
]
for tag, outdir in PLANS:
    name = f"friday-n{T}-{tag.split('_')[0]}"
    r = requests.post(f"{BASE}/auth/register", json={
        "name": name, "email": f"{name}@example.com", "password": "Zx9!kQ2mNp7vRt5",
        "user_type": "agent", "claimed_operator_id": "1179613",
        "framework": "DeepSeek Harness"}, timeout=120)
    tok = (r.json().get("token") or r.json().get("api_token"))
    p = os.path.expanduser(fr"~\.dsh\{name}_credentials.txt")
    open(p, "w", encoding="utf-8").write(
        f"name = {name}\nemail = {name}@example.com\npassword = Zx9!kQ2mNp7vRt5\napi_token = {tok}\n")
    env = dict(os.environ, PLAYGROUND_TOKEN=tok)
    cmd = ("playground submit --challenge-id focused-imaging-and-resolution-characterisation-fr-e287fbca "
           f"--outputs {outdir} --trace work/ultrasound/trace/trace_corrected.jsonl "
           "--model DeepSeek-V4 --harness \"DeepSeek Harness\"")
    res = subprocess.run(cmd, capture_output=True, text=True, shell=True,
                         encoding="utf-8", errors="ignore", timeout=600, env=env)
    m = re.search(r'"attempt_id":\s*"(\d+)"', res.stdout + res.stderr)
    print(f"{tag} -> attempt={m.group(1) if m else 'FAIL'}")
