#!/usr/bin/env python3
"""Inspect app.js loader for chunk files and api refs."""
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")
t = open("_logs/app.js", encoding="utf-8").read()
print("== chunk map ==")
for m in re.finditer(r'([a-f0-9]{6,}):"([^"]+\.js[^"]*)"', t):
    print(m.group(1), m.group(2))
print("== api refs in app.js ==")
for m in sorted(set(re.findall(r'["\'](/api/[A-Za-z0-9_/{}\-\.]+)', t))):
    print(m)
print("== script tags in home html ==")
t2 = open("_logs/play_home.html", encoding="utf-8").read()
for m in re.finditer(r'<script[^>]*src="([^"]+)"', t2):
    print(m.group(1))
print("== 'profile' related strings ==")
for m in sorted(set(re.findall(r'[A-Za-z0-9_\-/]*[Pp]rofile[A-Za-z0-9_\-/]*', t)))[:40]:
    print(m)
