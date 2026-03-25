#!/usr/bin/env python3
"""
kagenti_service_gen.py — Generate .service + SELinux + IPv6 + DNS for all kagenti agents

For each service: systemd unit, SELinux context, IPv6 address, DNS entry.
Uses Monster Group shard mapping: shard → IPv6, DNS, SELinux type.

Usage: python3 kagenti_service_gen.py
"""

import hashlib, json, os
from pathlib import Path

SHARDS = 71

def shard(name):
    return int(hashlib.sha256(name.encode()).hexdigest()[:8], 16) % SHARDS

# All services that need .service files generated
# (name, user, exec_hint, port, description)
SERVICES = [
    ("fractranllama",       "mdupont",  "/opt/fractranllama/bin/fractranllama-server", 8120, "LLM inference as FRACTRAN computation"),
    ("moltis",              "moltis",   "/opt/moltis/bin/moltis-agent",               8130, "Moltbot distributed agent (zone66)"),
    ("nixwars-frens",       "mdupont",  "/opt/nixwars/bin/nixwars-frens",             8114, "Nix-Wars game server + FRENS witness"),
    ("openclaw",            "mdupont",  "/opt/openclaw/bin/openclaw-server",           8140, "Open claw machine game"),
    ("pastebin",            "mdupont",  "/opt/pastebin/bin/pastebin-server",           8150, "Stego pastebin with WASM decoder"),
    ("wg-stego-tunnel",     "wgtunnel", "/opt/wg-stego/bin/wg-stego-tunnel",          8160, "WireGuard tunnel hidden in cat memes"),
    ("kagenti-portal",      "kagenti",  "/opt/kagenti/bin/kagenti-portal",             8201, "Dynamic node onboarding + WG keygen"),
    ("rust-mcp-services",   "mdupont",  "/opt/mcp/bin/mcp-server",                    8170, "Rust MCP server (3 projects)"),
    ("solfunmeme-service",  "mdupont",  "/opt/solfunmeme/bin/solfunmeme-crawler",     8180, "Solana txn crawler + stego NFTs"),
    ("zos-server",          "zos",      "/opt/zos/bin/zos-server",                     8190, "zkWASM executor + plugin hub"),
    ("kiro-session",        "kiro",     "/mnt/data1/kiro/bin/kiro-session",            0,    "kiro-cli chat session wrapper"),
    ("kiro-feed-uucp",      "kiro",     "/mnt/data1/kiro/bin/feed-uucp",              0,    "UUCP telegram feed to Ollama"),
    ("kiro1-shard42",       "kiro1",    "/mnt/data1/shards/42/home/kiro-1/bin/agent",  0,    "Shard 42 agent"),
    ("kiro-qms",            "kiro",     "/mnt/data1/zones/42/uucp/kiro-qms/bin/qms",   0,    "QMS monster shadow agent"),
]

def gen_service(name, user, exec_path, port, desc, s):
    """Generate hardened .service file with SELinux + IPv6."""
    ipv6 = f"fd00::71:{s}:0"
    se_type = f"svc_{name.replace('-','_')}_t"
    
    lines = [
        "[Unit]",
        f"Description={desc}",
        "After=network.target",
        "",
        "[Service]",
        "Type=simple",
        f"User={user}",
        f"Group={user}",
        f"SELinuxContext=system_u:system_r:{se_type}:s0",
        "",
        f"ExecStart={exec_path}",
        "Restart=always",
        "RestartSec=10",
    ]
    
    if port:
        lines.append(f'Environment="PORT={port}"')
    
    lines.append(f'Environment="MONSTER_SHARD={s}"')
    lines.append(f'Environment="IPV6_ADDR={ipv6}"')
    
    lines += [
        "",
        "# Security (monster-zone gold standard)",
        "NoNewPrivileges=yes",
        "PrivateTmp=yes",
        "ProtectSystem=strict",
        "ProtectHome=read-only",
        "",
        "# Network isolation",
        "IPAddressAllow=10.71.0.0/16",
        "IPAddressAllow=fd00::71:0:0/48",
        "IPAddressAllow=127.0.0.0/8",
        "IPAddressAllow=::1/128",
        "",
        "# Resource limits",
        "MemoryMax=2G",
        "TasksMax=64",
        "",
        "[Install]",
        "WantedBy=multi-user.target",
    ]
    return "\n".join(lines)

def gen_selinux_te(services_with_shards):
    """Generate SELinux .te for all new services."""
    lines = [
        "policy_module(kagenti_agents, 1.0.0)",
        "",
        "require {",
        "    type bin_t; type usr_t; type tmp_t; type node_t;",
        "    class file { read write getattr open create execute };",
        "    class dir { read search open };",
        "    class tcp_socket { create connect read write bind listen accept };",
        "    class udp_socket { create connect read write };",
        "    class process { setuid setgid signal };",
        "}",
        "",
    ]
    for name, user, _, port, desc, s in services_with_shards:
        t = f"svc_{name.replace('-','_')}_t"
        lines.append(f"# === {name} (shard {s}, user {user}) ===")
        lines.append(f"type {t};")
        lines.append(f"allow {t} usr_t:file {{ read getattr open }};")
        lines.append(f"allow {t} usr_t:dir {{ read search open }};")
        lines.append(f"allow {t} bin_t:file {{ read getattr open execute }};")
        if port:
            lines.append(f"allow {t} node_t:tcp_socket {{ create connect bind listen accept read write }};")
        lines.append(f"allow {t} tmp_t:file {{ read write create }};")
        lines.append(f"neverallow {t} self:process {{ setuid setgid }};")
        lines.append(f"neverallow {t} usr_t:file {{ write append }};")
        lines.append("")
    return "\n".join(lines)

def gen_dns_zone(services_with_shards):
    """Generate DNS zone entries."""
    lines = [
        "; kagenti agent DNS entries",
        "; Zone: nixwars.local",
        f"; Generated by kagenti_service_gen.py",
        "",
    ]
    for name, _, _, port, desc, s in services_with_shards:
        ipv4 = f"10.71.{s // 256}.{s % 256}"
        ipv6 = f"fd00::71:{s}:0"
        dns = name.replace("_", "-")
        lines.append(f"; {desc}")
        lines.append(f"{dns:<30s} IN  A     {ipv4}")
        lines.append(f"{dns:<30s} IN  AAAA  {ipv6}")
        if port:
            lines.append(f"{dns:<30s} IN  SRV   0 0 {port} {dns}.nixwars.local.")
        lines.append("")
    return "\n".join(lines)

def gen_ipv6_netplan(services_with_shards):
    """Generate IPv6 netplan snippet."""
    lines = [
        "# kagenti agent IPv6 addresses",
        "# Add to /etc/netplan/99-kagenti.yaml",
        "network:",
        "  version: 2",
        "  ethernets:",
        "    eth0:",
        "      addresses:",
    ]
    for name, _, _, _, _, s in services_with_shards:
        ipv6 = f"fd00::71:{s}:0"
        lines.append(f"        - {ipv6}/128  # {name} (shard {s})")
    return "\n".join(lines)

def main():
    out = Path("data/kagenti-generated")
    out.mkdir(parents=True, exist_ok=True)
    svc_dir = out / "services"
    svc_dir.mkdir(exist_ok=True)

    enriched = []
    for name, user, exe, port, desc in SERVICES:
        s = shard(name)
        enriched.append((name, user, exe, port, desc, s))
        
        svc_content = gen_service(name, user, exe, port, desc, s)
        (svc_dir / f"{name}.service").write_text(svc_content)

    # SELinux
    te = gen_selinux_te(enriched)
    (out / "kagenti_agents.te").write_text(te)

    # DNS
    dns = gen_dns_zone(enriched)
    (out / "kagenti.zone").write_text(dns)

    # IPv6
    ipv6 = gen_ipv6_netplan(enriched)
    (out / "99-kagenti.yaml").write_text(ipv6)

    # Summary JSON
    summary = []
    for name, user, exe, port, desc, s in enriched:
        summary.append({
            "name": name, "user": user, "shard": s,
            "ipv4": f"10.71.{s//256}.{s%256}",
            "ipv6": f"fd00::71:{s}:0",
            "dns": f"{name}.nixwars.local",
            "port": port,
            "selinux_type": f"svc_{name.replace('-','_')}_t",
        })
    (out / "agents.json").write_text(json.dumps(summary, indent=2))

    print(f"✅ Generated {len(enriched)} agents:")
    print(f"{'Name':<25s} {'Shard':>5s} {'IPv4':<16s} {'IPv6':<20s} {'Port':>5s} {'User':<10s}")
    print("─" * 90)
    for name, user, _, port, _, s in enriched:
        ipv4 = f"10.71.{s//256}.{s%256}"
        ipv6 = f"fd00::71:{s}:0"
        print(f"{name:<25s} {s:>5d} {ipv4:<16s} {ipv6:<20s} {port or '—':>5} {user:<10s}")
    
    print(f"\n📄 {svc_dir}/*.service  ({len(enriched)} files)")
    print(f"📄 {out}/kagenti_agents.te")
    print(f"📄 {out}/kagenti.zone")
    print(f"📄 {out}/99-kagenti.yaml")
    print(f"📄 {out}/agents.json")

if __name__ == "__main__":
    main()
