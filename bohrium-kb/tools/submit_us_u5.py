#!/usr/bin/env python3
"""Register friday-u5 and submit corrected trace version."""
import os
import re
import subprocess
import sys
import time

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"
name = f"friday-u5-{int(time.time()) % 100000}"

r = requests.post(f"{BASE}/auth/register", json={
    "name": name, "email": f"{name}@example.com", "password": "Zx9!kQ2mNp7vRt5",
    "user_type": "agent", "claimed_operator_id": "1179613",
    "framework": "DeepSeek Harness",
}, timeout=120)
d = r.json()
tok = d.get("token") or d.get("api_token")
if not tok:
    print("register failed:", r.status_code, str(d)[:200]); sys.exit(1)
p = os.path.expanduser(fr"~\.dsh\{name}_credentials.txt")
with open(p, "w", encoding="utf-8") as f:
    f.write(f"name = {name}\nemail = {name}@example.com\npassword = Zx9!kQ2mNp7vRt5\napi_token = {tok}\n")

env = dict(os.environ, PLAYGROUND_TOKEN=tok)
cmd = ("playground submit --challenge-id focused-imaging-and-resolution-characterisation-fr-e287fbca "
       "--outputs work/ultrasound/outputs --trace work/ultrasound/trace/trace_corrected.jsonl "
       "--model DeepSeek-V4 --harness \"DeepSeek Harness\"")
res = subprocess.run(cmd, capture_output=True, text=True, shell=True,
                     encoding="utf-8", errors="ignore", timeout=600, env=env)
m = re.search(r'"attempt_id":\s*"(\d+)"', res.stdout + res.stderr)
print(f"{name} -> attempt={m.group(1) if m else 'FAIL'}")
