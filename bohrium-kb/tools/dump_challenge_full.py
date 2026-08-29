#!/usr/bin/env python3
"""Dump full challenge JSON (content/scoring/outputs) for R3 Q10 to a file."""
import json, os, re, sys
import requests
sys.stdout.reconfigure(encoding="utf-8")
cred=open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"),encoding="utf-8").read()
TOKEN=re.search(r"api_token\s*=\s*(\S+)",cred).group(1)
H={"Authorization":f"Bearer {TOKEN}"}
BASE="https://play.bohrium.com/api"
SLUG="spatial-domain-identification-via-graph-informed-c-35985da3-2"
d=requests.get(f"{BASE}/challenges/{SLUG}",headers=H,timeout=60).json()
out=r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\bohrium-kb\round3_prep\challenge_10_full.json"
json.dump(d,open(out,"w",encoding="utf-8"),ensure_ascii=False,indent=1,default=str)
print("WROTE",out)
print("top keys:",sorted(d.keys()))
for k in ["scoring","formula","evaluation","outputs","expected_outputs","baseline","reference","target","table"]:
    if k in d:
        print("\n###",k,":",json.dumps(d[k],ensure_ascii=False,indent=1)[:1500])
