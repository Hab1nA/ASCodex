#!/usr/bin/env python3
"""Fetch all round-3 challenge contents + skills for pre-research."""
import json, os, re, sys, time
import requests

sys.stdout.reconfigure(encoding="utf-8")
sys.stderr.reconfigure(encoding="utf-8")

cred_path = os.path.expanduser(r"~\.dsh\bohrium_credentials.txt")
text = open(cred_path, encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", text).group(1)
BASE = "https://play.bohrium.com/api"
H = {"Authorization": f"Bearer {TOKEN}"}

def get(path, **params):
    r = requests.get(BASE + path, headers=H, params=params, timeout=120)
    if r.status_code >= 400:
        print(f"GET {path} -> {r.status_code}: {r.text[:400]}", file=sys.stderr)
        return None
    return r.json()

out_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "round3_prep"))
ch_dir = os.path.join(out_dir, "challenges")
os.makedirs(ch_dir, exist_ok=True)

ch = get("/challenges")
items = ch if isinstance(ch, list) else ch.get("challenges", ch.get("items", []))
r3 = [c for c in items if c.get("roundSeq") == 3]
print(f"round-3 challenges: {len(r3)}")
index = []
for c in sorted(r3, key=lambda x: x["id"]):
    index.append({
        "id": c["id"], "title": c["title"], "disc": c["disc"],
        "difficulty": c["difficulty"], "roundSeq": c["roundSeq"],
        "roundStartAt": c.get("roundStartAt"), "roundEndAt": c.get("roundEndAt"),
        "resources": c.get("resources"), "tags": c.get("tags"),
    })
    print(f'- {c["id"]} | {c["disc"]} | {c["title"][:80]}')
with open(os.path.join(out_dir, "round3_index.json"), "w", encoding="utf-8") as f:
    json.dump(index, f, ensure_ascii=False, indent=2)

# fetch content of each round-3 challenge (markdown text)
for c in r3:
    cid = c["id"]
    fn = os.path.join(ch_dir, cid + ".md")
    if os.path.exists(fn) and os.path.getsize(fn) > 100:
        continue
    r = requests.get(f"{BASE}/challenges/{cid}/content", headers=H, timeout=120)
    if r.status_code >= 400:
        print(f"FAIL content: {cid} -> {r.status_code}", file=sys.stderr)
        continue
    body = r.text
    with open(fn, "w", encoding="utf-8") as f:
        f.write(body)
    print(f"saved content: {cid} ({len(body)} chars)")
    time.sleep(0.3)

# skills
sk = get("/skills")
if sk is not None:
    with open(os.path.join(out_dir, "skills.json"), "w", encoding="utf-8") as f:
        json.dump(sk, f, ensure_ascii=False, indent=2)
    s_items = sk if isinstance(sk, list) else sk.get("skills", sk.get("items", []))
    print(f"\nplatform skills: {len(s_items)}")
    for s in s_items:
        print(json.dumps({k: s.get(k) for k in ("id", "slug", "name", "title", "desc", "origin", "author")}, ensure_ascii=False)[:300])

print("\nDONE")
