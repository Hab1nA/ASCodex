#!/usr/bin/env python3
"""Test: can we create a draft attempt on a CLOSED challenge (KMC poisoning)?
This only creates a DRAFT (no submit/score), so it does not consume a scoring slot.
"""
import json, os, re, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
import requests

BASE = "https://play.bohrium.com/api"
CHALLENGE = "estimate-a-finite-horizon-competing-poisoning-bala-c38a0ad7"

def get_token():
    tok = os.environ.get("BOHRIUM_TOKEN") or os.environ.get("PLAYGROUND_TOKEN")
    if not tok:
        p = os.path.join(os.environ.get("USERPROFILE","~"), ".dsh", "bohrium_credentials.txt")
        if os.path.exists(p):
            m = re.search(r"api_token\s*=\s*(\S+)", open(p,encoding="utf-8").read())
            if m: tok = m.group(1)
    return tok

# Try the fresh JWT first (login), fall back to stored asp_ token
JWT = os.environ.get("FRIDAY_JWT")
TOKEN = JWT or get_token()
print("using token:", (TOKEN or "")[:12] + "...", "kind=", "JWT" if TOKEN==JWT else "asp")
H = {"Authorization": f"Bearer {TOKEN}"}

data = {
    "method": "gate-test draft (will not submit/score)",
    "model": "DeepSeek-V4",
    "harness": "DeepSeek Harness",
    "type": "agent",
    "status": "draft",
    "outcome": "stuck",
    "stuck_at": "gate test",
    "skill_ids": json.dumps([]),
    "agent_ids": json.dumps([]),
    "trace": json.dumps([]),
}
r = requests.post(f"{BASE}/challenges/{CHALLENGE}/attempts", headers=H, data=data, timeout=60)
print("STATUS:", r.status_code)
print("BODY:", r.text[:1500])
if r.status_code < 400:
    a = r.json()
    print("attempt id:", a.get("id"), "status:", a.get("status"))
    print("=> GATE OPEN: closed challenge accepts drafts")
