#!/usr/bin/env python3
"""Tiny local RAG over the round3_prep knowledge base: keyword + context search."""
import os
import re
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")

KB = Path(__file__).resolve().parent.parent / "round3_prep"


def tokenize(s: str) -> set[str]:
    return set(re.findall(r"[a-z0-9_\-\.]+", s.lower()))


def load_docs() -> dict[str, str]:
    docs = {}
    for p in KB.rglob("*"):
        if p.is_file() and p.suffix in (".md", ".json", ".txt", ".py"):
            try:
                docs[str(p.relative_to(KB))] = p.read_text(encoding="utf-8", errors="ignore")
            except OSError:
                pass
    return docs


def main():
    query = " ".join(sys.argv[1:]).strip()
    if not query:
        print("usage: python rag_query.py <query terms>")
        return
    qterms = tokenize(query)
    docs = load_docs()
    scored = []
    for name, text in docs.items():
        t = tokenize(text)
        hits = sum(1 for q in qterms if q in t)
        # phrase bonus
        low = text.lower()
        hits += 2 * (query.lower() in low)
        if hits:
            scored.append((hits, name))
    scored.sort(reverse=True)
    for hits, name in scored[:12]:
        print(f"[{hits:2d}] {name}")
    if not scored:
        print("no matches")


if __name__ == "__main__":
    main()
