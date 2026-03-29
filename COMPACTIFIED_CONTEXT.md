# Compactified Context

## 2026-03-27

- Source: current working turn
- Main decision:
  - `zkperf` is a credible upstream home for a small sample-trace compaction
    utility because the work is directly about zkperf-derived perf/sample trace
    payloads, not FRACDASH-specific bridge semantics
  - the upstreamable slice should stay narrow:
    - standalone script
    - standalone regression script
    - README/doc note
  - do not upstream the broader FRACDASH motif/compression stack until a small
    generic utility lands cleanly and proves useful on richer zkperf trace
    families
- Intended behavior:
  - treat normalized sample-trace JSON as a projection/reconstruction contract
  - preserve only the raw generating fields needed to rebuild deterministic
    derived fields
  - require exact round-trip before treating the compact form as valid
