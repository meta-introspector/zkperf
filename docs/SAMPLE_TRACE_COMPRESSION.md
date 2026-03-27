# Sample Trace Compression

This note records the first small upstream utility for compacting normalized
sample-trace JSON.

## Scope

The utility is intentionally narrow:

- input: normalized sample-trace JSON
- output: compact JSON that preserves only the raw fields needed to reconstruct
  the derived trace surface exactly

It does **not** attempt to be a generic byte-level compressor.

## Compression model

The current model is projection plus exact reconstruction:

- keep persistent generating fields
- drop deterministic derived fields
- rebuild the dropped fields on decode

For the current normalized sample trace shape, the compact form keeps:

- `step`
- `event_idx`
- `timestamp`
- `period`
- `pid`
- `tid`
- `cpu_mode`
- `cid`

and drops derived fields such as:

- expanded matrix rows
- expanded annotation deltas
- other deterministic rebuild products

## Why this is useful

This gives a concrete, auditable compression witness:

- the compact payload is smaller
- the round-trip contract is exact
- the gain comes from removing duplicated derived structure rather than hiding
  lossy approximations inside a general-purpose codec

## Current boundary

This is a utility for normalized sample-trace JSON only.

If later trace families need stronger gains, the next layer should be richer
motif grammars or family-specific model contracts, not unprincipled additional
compression stages.
