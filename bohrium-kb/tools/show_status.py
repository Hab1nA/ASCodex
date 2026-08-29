#!/usr/bin/env python3
import json

p = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\_logs\cand_details.json"
d = json.load(open(p, encoding="utf-8"))
for x in sorted(d, key=lambda r: -(r.get("attempts") or 0)):
    title = (x.get("title") or x.get("id"))[:78]
    print(f"{str(x.get('attempts')):>4} att | {str(x.get('status')):<8} | d={x.get('difficulty')} | ours={len(x.get('our_attempts') or [])} | {title}")
