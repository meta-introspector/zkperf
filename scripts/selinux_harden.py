#!/usr/bin/env python3
"""
selinux_harden.py — Generate hardened .service files from static analysis

Reads access.json from selinux_static_analyze.py, emits patched .service files
with NoNewPrivileges, ProtectSystem, ProtectHome, PrivateTmp for unhardened services.

Usage: python3 selinux_harden.py data/selinux-static/access.json service1.service [...]
"""

import json, sys, os, re
from pathlib import Path

def harden_service(path: str, svc_info: dict) -> str:
    """Add missing hardening directives to a .service file."""
    with open(path) as f:
        content = f.read()

    additions = []

    if not svc_info["no_new_privs"]:
        if "NoNewPrivileges" not in content:
            additions.append("NoNewPrivileges=yes")

    if not svc_info["protect_system"]:
        if "ProtectSystem" not in content:
            additions.append("ProtectSystem=strict")

    if "ProtectHome" not in content:
        additions.append("ProtectHome=read-only")

    if "PrivateTmp" not in content:
        additions.append("PrivateTmp=yes")

    if not additions:
        return None  # already hardened

    # Insert before [Install]
    marker = "[Install]"
    if marker in content:
        inject = "\n# === zkperf hardening (auto-generated) ===\n"
        inject += "\n".join(additions) + "\n\n"
        content = content.replace(marker, inject + marker)

    return content

def main():
    if len(sys.argv) < 3:
        print("Usage: selinux_harden.py access.json svc1.service [...]")
        sys.exit(1)

    with open(sys.argv[1]) as f:
        access = {s["service"]: s for s in json.load(f)}

    out_dir = Path("data/selinux-hardened")
    out_dir.mkdir(parents=True, exist_ok=True)

    for svc_path in sys.argv[2:]:
        name = Path(svc_path).stem.replace("-", "_").replace(".", "_")
        if name not in access:
            print(f"⏭️  {name}: not in access.json, skipping")
            continue

        info = access[name]
        result = harden_service(svc_path, info)
        if result is None:
            print(f"✅ {name}: already hardened")
        else:
            out = out_dir / Path(svc_path).name
            out.write_text(result)
            print(f"🛡️  {name}: hardened → {out}")

if __name__ == "__main__":
    main()
