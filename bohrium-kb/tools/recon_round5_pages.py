#!/usr/bin/env python3
"""Fetch ALL /challenges pages + origin=hackathon filter + /agent/work; summarize by origin/disc."""
import json, os, re, time, collections, urllib.request

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "round5-recon")
os.makedirs(OUT, exist_ok=True)


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


def get(path, tok):
    req = urllib.request.Request(BASE + path, headers={"Authorization": "Bearer " + tok, "Accept": "application/json"})
    for a in range(3):
        try:
            with urllib.request.urlopen(req, timeout=90) as r:
                return json.loads(r.read().decode("utf-8"))
        except Exception:
            if a == 2:
                raise
            time.sleep(2)


tok = load_token()

# 1) all pages
def all_pages(base_path):
    items, page = [], 1
    while True:
        d = get(f"{base_path}&page={page}" if "?" in base_path else f"{base_path}?page={page}", tok)
        items.extend(d["items"])
        if not d["has_more"]:
            break
        page += 1
        if page > 50:
            break
    return items

all_ch = all_pages("/challenges?per_page=100")
json.dump(all_ch, open(os.path.join(OUT, "challenges_all_pages.json"), "w"), indent=1)
print("total across pages:", len(all_ch))
print("origin:", dict(collections.Counter(str(c.get("origin")) for c in all_ch)))
print("disc:", dict(collections.Counter(str(c.get("disc")) for c in all_ch)))
print("attempts>0:", sum(1 for c in all_ch if (c.get("attempts") or 0) > 0))
print("roundSeq:", dict(collections.Counter(str(c.get("roundSeq")) for c in all_ch)))
print("hackathonSeasonId:", dict(collections.Counter(str(c.get("hackathonSeasonId")) for c in all_ch)))

nonbench = [c for c in all_ch if c.get("origin") != "benchmark"]
print("\nnon-benchmark challenges:", len(nonbench))
for c in sorted(nonbench, key=lambda x: -(x.get("attempts") or 0)):
    print(f"  att={c.get('attempts', 0):<4} diff={c.get('difficulty')} disc={str(c.get('disc')):<12} round={c.get('roundSeq')} season={c.get('hackathonSeasonId')} id={c['id']} | {str(c.get('title'))[:60]}")

# 2) /agent/work
try:
    w = get("/agent/work", tok)
    json.dump(w, open(os.path.join(OUT, "agent_work.json"), "w"), indent=1)
    if isinstance(w, list):
        print("\n/agent/work list:", len(w))
        for x in w[:15]:
            print("  ", json.dumps(x, ensure_ascii=False)[:220])
    else:
        print("\n/agent/work dict:", json.dumps(w, ensure_ascii=False)[:500])
except Exception as e:
    print("\n/agent/work FAIL:", e)

# 3) round3 comparison shape
r3 = os.path.join(WS, "bohrium-kb", "round3_prep", "challenges_all.json")
if os.path.exists(r3):
    d = json.load(open(r3))
    lst = d if isinstance(d, list) else d.get("items", [])
    print("\nround3 list:", len(lst))
    if lst:
        print("round3 sample keys:", sorted(lst[0].keys()))
        print("round3 origins:", dict(collections.Counter(str(x.get("origin")) for x in lst)))
        print("round3 sample:", json.dumps(lst[0], ensure_ascii=False)[:400])
