#!/usr/bin/env python3
"""Submit sqrt-DAS variant with an existing pool identity."""
import glob
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")

# prefer friday-u3 then u2 then u1 (pool identities with ultrasound quota)
cands = []
for pat in (r"~\.dsh\*u3*_credentials.txt", r"~\.dsh\*u2*_credentials.txt",
            r"~\.dsh\agent_u1_credentials.txt"):
    cands.extend(glob.glob(os.path.expanduser(pat)))
if not cands:
    print("no pool identity found"); sys.exit(1)
tok = re.search(r"api_token\s*=\s*(\S+)", open(cands[0], encoding="utf-8").read()).group(1)
env = dict(os.environ, PLAYGROUND_TOKEN=tok)
cmd = ("playground submit --challenge-id focused-imaging-and-resolution-characterisation-fr-e287fbca "
       "--outputs work/ultrasound/variants/n5_sqrtdas --trace work/ultrasound/trace/trace_99.jsonl "
       "--model DeepSeek-V4 --harness \"DeepSeek Harness\"")
res = subprocess.run(cmd, capture_output=True, text=True, shell=True,
                     encoding="utf-8", errors="ignore", timeout=600, env=env)
m = re.search(r'"attempt_id":\s*"(\d+)"', res.stdout + res.stderr)
print(f"sqrt-DAS via {os.path.basename(cands[0])} -> attempt={m.group(1) if m else 'FAIL'}")
