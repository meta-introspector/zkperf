# FRACTRAN eRDFa Stream Report — All Pastebin Chats

**Date:** 2026-03-20
**Source:** /mnt/data1/spool/uucp/pastebin/ (1,366 text files)

## Pipeline

1. `fractran-vm reflect <file>` → .dasl cache (DA51 shape + FRACTRAN profile)
2. `fractran_erdfa_dynamic <file>` → erdfa# datagram (prime-encoded HTML structure)

## Results

| Metric | Value |
|--------|-------|
| Files processed | 1,366 |
| .dasl caches created | 1,366 |
| eRDFa datagrams | 1,366 |
| Active fractran states (>1) | 925 (67.7%) |
| Empty states (=1, no HTML) | 441 (32.3%) |

## FRACTRAN Step Distribution

| Steps | Files | Description |
|-------|-------|-------------|
| 0 | 376 | No FRACTRAN fractions matched (plain text) |
| 1 | 155 | Single reduction |
| 2 | 19 | |
| 3 | 353 | Common for structured text |
| 4 | 325 | Common for HTML-bearing pastes |
| 5 | 68 | |
| 6+ | 70 | Complex structure (up to 10 steps) |

## Top DA51 Addresses (Recurring Shapes)

| DA51 Address | Count | Interpretation |
|-------------|-------|----------------|
| 0xDA511E0028000011 | 104 | Most common shape class |
| 0xDA511E0028000017 | 53 | |
| 0xDA511E002800001A | 26 | |
| 0xDA511E0028000019 | 20 | |
| 0xDA511E804000008F | 11 | |

## Largest Shapes (by node count)

| Nodes | Lists | Symbols | Depth | Arena |
|-------|-------|---------|-------|-------|
| 22,106 | 659 | 20,509 | 2 | 352KB |
| 16,427 | 16 | 16,384 | 2 | 263KB |
| 14,845 | 43 | 13,998 | 5 | 231KB |
| 12,964 | 462 | 11,928 | 2 | 208KB |
| 12,035 | 1,793 | 9,450 | 8 | 210KB |

## Top FRACTRAN States

| State (hex) | Count | Notes |
|-------------|-------|-------|
| 20 | 257 | Minimal HTML (single tag) |
| ffffffff...f | 164 | Overflow / max state |
| 1da02dfd49ffece0 | 61 | Rich HTML structure |
| 4dc4b8edc78e | 58 | eRDFa-bearing pastes |
| 1e60 | 44 | |

## Files

- `reflect-log.txt` — Full fractran-vm reflect output
- `erdfa-datagrams.txt` — All 1,366 erdfa# datagram lines
- `.dasl` files in pastebin dir — Individual FRACTRAN caches
