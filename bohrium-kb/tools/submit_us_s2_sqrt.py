#!/usr/bin/env python3
"""Land the sqrt-DAS variant under the summary identity friday-s2."""
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")

cred = open(os.path.expanduser(r"~\.dsh\agent2_credentials.txt"), encoding="utf-8").read()
tok = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
env = dict(os.environ, PLAYGROUND_TOKEN=tok)
cmd = ("playground submit --challenge-id focused-imaging-and-resolution-characterisation-fr-e287fbca "
       "--outputs work/ultrasound/variants/n5_sqrtdas --trace work/ultrasound/trace/trace_99.jsonl "
       "--model DeepSeek-V4 --harness \"DeepSeek Harness\"")
res = subprocess.run(cmd, capture_output=True, text=True, shell=True,
                     encoding="utf-8", errors="ignore", timeout=600, env=env)
m = re.search(r'"attempt_id":\s*"(\d+)"', res.stdout + res.stderr)
print("friday-s2 sqrt-DAS -> attempt", m.group(1) if m else "FAIL")
