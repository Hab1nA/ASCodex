#!/usr/bin/env python3
"""Sign a Guard policy file with the ASCodex trust-anchor private key.

The private key is read from ASCODEX_TRUST_ANCHOR_PRIVATE_KEY (path) or
ASCODEX_TRUST_ANCHOR_PRIVATE_KEY_PEM (PEM literal). It is never written into
this workspace or committed. The matching public key must equal the
compile-time ASCODEX_TRUST_ANCHOR_PUBLIC_KEY_HEX in solver-guard/src/lib.rs.

Usage:
    python scripts/ascodex_sign_policy.py <policy.yaml>
    # writes <policy.yaml>.sig containing the 64-byte Ed25519 signature as hex
"""

from __future__ import annotations

import hashlib
import os
import sys
from pathlib import Path

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import load_pem_private_key

EXPECTED_ANCHOR = (
    "769f138d63fa14ab907595f66142da266c8741e9ad1dc9a2eae2f4cf924cdf8e"
)


def load_private_key() -> Ed25519PrivateKey:
    pem_path = os.environ.get("ASCODEX_TRUST_ANCHOR_PRIVATE_KEY")
    pem = os.environ.get("ASCODEX_TRUST_ANCHOR_PRIVATE_KEY_PEM")
    if pem_path:
        key = load_pem_private_key(Path(pem_path).read_bytes(), password=None)
    elif pem:
        key = load_pem_private_key(pem.encode("utf-8"), password=None)
    else:
        raise SystemExit(
            "set ASCODEX_TRUST_ANCHOR_PRIVATE_KEY (path) or "
            "ASCODEX_TRUST_ANCHOR_PRIVATE_KEY_PEM to provision the signing key"
        )
    assert isinstance(key, Ed25519PrivateKey), "trust anchor key must be Ed25519"
    return key


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    policy_path = Path(sys.argv[1]).resolve()
    if not policy_path.is_file():
        print(f"policy file not found: {policy_path}", file=sys.stderr)
        return 2
    key = load_private_key()
    public_hex = key.public_key().public_bytes_raw().hex()
    if public_hex.lower() != EXPECTED_ANCHOR.lower():
        print(
            "private key does not match the compile-time trust anchor "
            f"(got {public_hex})",
            file=sys.stderr,
        )
        return 2
    policy_bytes = policy_path.read_bytes()
    digest = hashlib.sha256(policy_bytes).hexdigest()
    signature = key.sign(policy_bytes).hex()
    signature_path = policy_path.with_name(policy_path.name + ".sig")
    signature_path.write_text(signature + "\n", encoding="utf-8")
    print(f"signed {policy_path}")
    print(f"sha256: {digest}")
    print(f"signature: {signature_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
