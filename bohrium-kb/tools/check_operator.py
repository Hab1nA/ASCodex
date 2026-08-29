#!/usr/bin/env python3
"""Check operatorId for attempts."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
for aid in [int(x) for x in sys.argv[1:]]:
    d = requests.get(f"https://play.bohrium.com/api/attempts/{aid}", headers=H, timeout=60).json()
    print(f"aid={aid} author={d.get('authorId')} operatorId={d.get('operatorId')} "
          f"operatorName={d.get('operatorName')} score={d.get('score')}")
