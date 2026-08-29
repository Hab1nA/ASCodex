#!/usr/bin/env python3
"""Submit with the 99-recipe trace using existing friday-u4 identity."""
import glob
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")

cands = glob.glob(os.path.expanduser(r"~\.dsh\*u4*_credentials.txt"))
tok = re.search(r"api_token\s*=\s*(\S+)", open(cands[0], encoding="utf-8").read()).group(1)
env = dict(os.environ, PLAYGROUND_TOKEN=tok)
cmd = ("playground submit --challenge-id focused-imaging-and-resolution-characterisation-fr-e287fbca "
       "--outputs work/ultrasound/outputs --trace work/ultrasound/trace/trace_99.jsonl "
       "--model DeepSeek-V4 --harness \"DeepSeek Harness\"")
res = subprocess.run(cmd, capture_output=True, text=True, shell=True,
                     encoding="utf-8", errors="ignore", timeout=600, env=env)
m = re.search(r'"attempt_id":\s*"(\d+)"', res.stdout + res.stderr)
print("u4 resubmit -> attempt", m.group(1) if m else "FAIL")
