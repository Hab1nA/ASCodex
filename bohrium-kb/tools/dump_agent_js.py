#!/usr/bin/env python3
"""Dump the full agent-management JS section of app.js."""
import sys

sys.stdout.reconfigure(encoding="utf-8")
t = open("_logs/app.js", encoding="utf-8").read()
# the agent UI code is around 77000-85000; dump it fully
print(t[77000:86000])
