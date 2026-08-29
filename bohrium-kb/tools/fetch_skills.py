#!/usr/bin/env python3
"""Download platform skill specs (SKILL.md) into local knowledge base."""
import json, os, re, sys
import requests

sys.stdout.reconfigure(encoding="utf-8")

cred_path = os.path.expanduser(r"~\.dsh\bohrium_credentials.txt")
text = open(cred_path, encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", text).group(1)
BASE = "https://play.bohrium.com/api"
H = {"Authorization": f"Bearer {TOKEN}"}

WANT = [
    "reproduce-paper", "red-team-review", "proof-verify", "generate-grader",
    "bohrium-compute", "score-difficulty", "distill", "multi-agent-reproduce",
    "checkpoint", "resume", "bio-reproduce", "reproduce-submit",
    "grade-reproduction", "reproduce-validate",
]

out_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "round3_prep", "skills"))
os.makedirs(out_dir, exist_ok=True)

for slug in WANT:
    # try both slug endpoints
    got = False
    for path in (f"/skills/{slug}/spec", f"/skills/{slug}"):
        r = requests.get(BASE + path, headers=H, timeout=60)
        if r.status_code == 200:
            body = r.text
            if body.strip().startswith("{"):
                try:
                    j = r.json()
                    spec = j.get("spec") or j.get("content") or j.get("body") or j.get("skill", {}).get("spec")
                    if isinstance(spec, str):
                        body = spec
                    elif isinstance(spec, dict):
                        body = json.dumps(spec, ensure_ascii=False, indent=2)
                    else:
                        body = json.dumps(j, ensure_ascii=False, indent=2)
                except ValueError:
                    body = r.text
            fn = os.path.join(out_dir, slug + ".md")
            with open(fn, "w", encoding="utf-8") as f:
                f.write(body)
            print(f"OK   {slug} <- {path} ({len(body)} chars)")
            got = True
            break
    if not got:
        print(f"MISS {slug} (404 or error)")
