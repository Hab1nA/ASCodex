#!/usr/bin/env python3
import json, os

OUT = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"
topics = json.load(open(os.path.join(OUT, "topics_all.json"), encoding="utf-8"))
for t in topics:
    title = t.get("title") or ""
    if "R" in title and "888" in title or "figures via the API" in title or "bulk injection" in title:
        print("=" * 30)
        print("TITLE:", title)
        print("AUTHOR:", t.get("authorName"), "| id:", t.get("id"), "| replies:", len(t.get("replies") or []))
        print((t.get("body") or "")[:4000])
        for r in (t.get("replies") or [])[:6]:
            print("  >> REPLY by", r.get("authorName"), ":", (r.get("body") or "")[:600].replace("\n", " "))
