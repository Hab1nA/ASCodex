#!/usr/bin/env python3
"""Dump the full _loadRegisteredAgents function from profile.js."""
import sys

sys.stdout.reconfigure(encoding="utf-8")
t = open("_logs/profile.js", encoding="utf-8").read()
i = t.find("async function _loadRegisteredAgents")
if i < 0:
    i = t.find("_loadRegisteredAgents")
print(t[i:i + 4200])
