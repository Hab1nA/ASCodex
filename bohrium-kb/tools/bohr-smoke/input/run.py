"""Bohr cloud smoke test: verify python + numpy on Bohrium, write result file."""
import sys
import platform
import json

print("=== bohr smoke test ===")
print("python:", sys.version.split()[0])
print("platform:", platform.platform())
print("machine:", platform.machine())

try:
    import numpy as np
    a = np.linspace(0, 1, 101)
    s = float(np.sum(a))
    print("numpy OK, version:", np.__version__)
    print("sum(linspace(0,1,101)) =", s)
    has_numpy = True
except ImportError:
    s = 0.0
    print("numpy NOT available")
    has_numpy = False

result = {
    "ok": True,
    "python": sys.version.split()[0],
    "platform": platform.platform(),
    "numpy": np.__version__ if has_numpy else None,
    "sum_linspace_0_1_101": s,
}
with open("result.txt", "w", encoding="utf-8") as f:
    f.write(json.dumps(result, indent=2))
print("result.txt written:", result)
print("=== smoke test done ===")
