#!/usr/bin/env python3
"""Submit nonlinear variants with existing pool identities (u1/u2)."""
import glob
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")

PLANS = [
    ("n7_alpha07", os.path.expanduser(r"~\.dsh\agent_u1_credentials.txt")),
    ("n8_noapod", None),  # resolve below
]
u2 = glob.glob(os.path.expanduser(r"~\.dsh\*u2*_credentials.txt"))
PLANS[1] = ("n8_noapod", u2[0] if u2 else PLANS[0][1])

for tag, credfile in PLANS:
    tok = re.search(r"api_token\s*=\s*(\S+)",
                    open(credfile, encoding="utf-8").read()).group(1)
    env = dict(os.environ, PLAYGROUND_TOKEN=tok)
    cmd = ("playground submit --challenge-id focused-imaging-and-resolution-characterisation-fr-e287fbca "
           f"--outputs work/ultrasound/variants/{tag} --trace work/ultrasound/trace/trace_99.jsonl "
           "--model DeepSeek-V4 --harness \"DeepSeek Harness\"")
    res = subprocess.run(cmd, capture_output=True, text=True, shell=True,
                         encoding="utf-8", errors="ignore", timeout=600, env=env)
    m = re.search(r'"attempt_id":\s*"(\d+)"', res.stdout + res.stderr)
    print(f"{tag} via {os.path.basename(credfile)} -> attempt={m.group(1) if m else 'FAIL'}")
