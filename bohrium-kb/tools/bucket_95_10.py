#!/usr/bin/env python3
"""Characterize 0.95-0.995 bucket + full bucket: model, harbor, trace, outcome, time."""
import json, os, re, sys
import requests
sys.stdout.reconfigure(encoding="utf-8")
cred=open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"),encoding="utf-8").read()
TOKEN=re.search(r"api_token\s*=\s*(\S+)",cred).group(1)
H={"Authorization":f"Bearer {TOKEN}"}
BASE="https://play.bohrium.com/api"
SLUG="spatial-domain-identification-via-graph-informed-c-35985da3-2"

# fetch all attempts again (paginate)
seen={}; page=1; total=None
while True:
    r=requests.get(f"{BASE}/challenges/{SLUG}/attempts",params={"per_page":50,"page":page},headers=H,timeout=60).json()
    items=r.get("attempts") or []
    if total is None: total=r.get("total")
    if not items: break
    for a in items: seen[a["id"]]=a
    page+=1
    if len(seen)>= (total or 0) or page>40: break

scored=[a for a in seen.values() if a.get("score") is not None]
scored.sort(key=lambda a:a.get("score") or 0,reverse=True)

print(f"scored={len(scored)} total={total}")
# bucket 0.95-0.995
for a in scored:
    s=a.get("score") or 0
    if s>=95.0 and s<99.5:
        print(f"  aid={a['id']} score={s:.4f} status={a.get('status')} outcome={a.get('outcome')} at={(a.get('createdAt') or '')[:16]} author={str(a.get('authorId'))[:18]}")
