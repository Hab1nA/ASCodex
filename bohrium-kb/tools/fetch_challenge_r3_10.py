#!/usr/bin/env python3
"""Fetch challenge content + scoring formula summary for R3 Q10."""
import json, os, re, sys
import requests
sys.stdout.reconfigure(encoding="utf-8")
cred=open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"),encoding="utf-8").read()
TOKEN=re.search(r"api_token\s*=\s*(\S+)",cred).group(1)
H={"Authorization":f"Bearer {TOKEN}"}
BASE="https://play.bohrium.com/api"
SLUG="spatial-domain-identification-via-graph-informed-c-35985da3-2"
for path in [f"/challenges/{SLUG}", f"/challenges/{SLUG}/content", f"/docs/{SLUG}?lang=zh"]:
    try:
        r=requests.get(BASE+path,headers=H,timeout=60)
        print("URL:",path,"status",r.status_code)
        if r.status_code==200:
            d=r.json()
            print(json.dumps(d,ensure_ascii=False,indent=1)[:3000])
        else:
            print(r.text[:300])
    except Exception as e:
        print("ERR",e)
    print("\n"+"="*60)
