#!/usr/bin/env python3
"""Full raw detail dump for specific attempts (29108 ours + full-score 19145/18957) - focused on
scorecard structure, scoringDetails keys, resultsJson keys, execLog keys, trace top-level keys."""
import json, os, re, sys
import requests
sys.stdout.reconfigure(encoding="utf-8")
cred=open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"),encoding="utf-8").read()
TOKEN=re.search(r"api_token\s*=\s*(\S+)",cred).group(1)
H={"Authorization":f"Bearer {TOKEN}"}
BASE="https://play.bohrium.com/api"
for aid in [29108,29124,29139,19145,18957,18681,19092,18723]:
    d=requests.get(f"{BASE}/attempts/{aid}",headers=H,timeout=60).json()
    print("\n"+"="*70)
    print(f"aid={aid} name={d.get('author_name')} score={d.get('score')} outcome={d.get('outcome')}")
    # top-level keys
    print("top-level keys:", sorted(d.keys()))
    sc=d.get("scorecard") or {}
    print("scorecard keys:", sorted(sc.keys()))
    sd=d.get("scoringDetails") or {}
    print("scoringDetails:", json.dumps(sd,ensure_ascii=False,indent=1)[:1200])
    rj=d.get("resultsJson")
    print("resultsJson type:", type(rj).__name__)
    tl=d.get("trace")
    print("trace type:", type(tl).__name__, ("len="+str(len(tl)) if hasattr(tl,'__len__') else ""))
    if isinstance(tl,dict): print("trace keys:", sorted(list(tl.keys())[:40]))
    print("has_agentMetadata:", bool(d.get('agentMetadata')), bool(d.get('metadata')))
