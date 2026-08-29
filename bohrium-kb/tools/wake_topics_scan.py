#!/usr/bin/env python3
"""Save /topics, find wake-challenge topics and any data links."""
import json, os, re, urllib.request

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "wake-dg-fit")


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


tok = load_token()
req = urllib.request.Request(BASE + "/topics?limit=200", headers={"Authorization": "Bearer " + tok})
d = json.loads(urllib.request.urlopen(req, timeout=60).read().decode("utf-8"))
topics = d.get("topics", d if isinstance(d, list) else [])
json.dump(topics, open(os.path.join(OUT, "topics_all.json"), "w", encoding="utf-8"), indent=1, ensure_ascii=False)
print("topics:", len(topics))
for t in topics:
    body = (t.get("body") or "")
    title = t.get("title") or t.get("subject") or ""
    if re.search(r"wang|wake|field\.txt|double.?gaussian|尾流", body + " " + str(title), re.I):
        print("\n=== TOPIC:", title, "| id:", t.get("id"), "| author:", t.get("authorName"), "| challenge:", t.get("challengeId"))
        print(body[:1500])

# also dump the chun_cai data-availability topic fully
for t in topics:
    body = t.get("body") or ""
    if "paper2task_api" in body or "already present" in body:
        print("\n=== DATA-AVAILABILITY TOPIC:", t.get("title") or t.get("id"))
        print(body[:2000])
        break
