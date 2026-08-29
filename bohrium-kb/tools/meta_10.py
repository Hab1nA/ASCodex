#!/usr/bin/env python3
"""Pull traceCount/method/harness/cpuHours/modelTag for key attempts."""
import json, os, re, sys
import requests
sys.stdout.reconfigure(encoding="utf-8")
cred=open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"),encoding="utf-8").read()
TOKEN=re.search(r"api_token\s*=\s*(\S+)",cred).group(1)
H={"Authorization":f"Bearer {TOKEN}"}
BASE="https://play.bohrium.com/api"
ids=[19145,18957,19092,18681,18898,19074,17996,18002,18724,23035,19297,18723,19325,18627,19255,18092]
for aid in ids:
    d=requests.get(f"{BASE}/attempts/{aid}",headers=H,timeout=60).json()
    sc=d.get("scorecard") or {}
    print(f"aid={aid} name={d.get('author_name')} score={d.get('score')} "
          f"harbor={sc.get('harbor_reward')} trace={sc.get('trace_score')} "
          f"model={d.get('modelTag')} method={str(d.get('method'))[:40]} "
          f"harness={str(d.get('harness'))[:20]} cpuH={d.get('cpuHours')} "
          f"traceCount={d.get('traceCount')} agentFramework={d.get('agentFramework')}")
