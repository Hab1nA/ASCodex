#!/usr/bin/env python3
"""Probe ground-state-shell-occupations problem post-season judge signals."""
import json, os, re, sys, time
import requests
sys.stdout.reconfigure(encoding="utf-8")
sys.stderr.reconfigure(encoding="utf-8")

cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

SLUG = "ground-state-shell-occupations-and-fbd-universal-3-2e27dc1a"

def get(path, **params):
    r = requests.get(BASE + path, headers=H, params=params, timeout=120)
    if r.status_code >= 400:
        print(f"GET {path} -> {r.status_code}: {r.text[:600]}", file=sys.stderr)
        return None
    return r.json()

# 1) leaderboard
lb = get("/leaderboard", challenge=SLUG, per_page=1000)
print("LB typeof:", type(lb))
print("LB head:", json.dumps(lb, ensure_ascii=False)[:1500])

print("\n====/challenges/{slug}/attempts====")
at = get(f"/challenges/{SLUG}/attempts", per_page=1000)
print("attempts typeof:", type(at))
if isinstance(at, dict):
    print("keys:", list(at.keys()))
    for k in at:
        v = at[k]
        print(f"  {k}: {json.dumps(v, ensure_ascii=False)[:800]}")
