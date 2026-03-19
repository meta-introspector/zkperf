#!/usr/bin/env node
/**
 * zkperf-verify.js — Verify a zkPerf witness chain.
 *
 * A reproducible perf witness needs the full chain:
 *   1. nix package of binaries
 *   2. source + debug symbols
 *   3. traces produced with binaries
 *   4. models created from traces
 *   5. events leading up to binary creation
 *
 * This verifies that all layers are present and the commitment matches.
 */
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');

function verifyChain(chainFile) {
  const chain = JSON.parse(fs.readFileSync(chainFile, 'utf8'));
  const layers = chain.layers;
  const results = [];

  // Check all 5 layers present
  const required = ['1_binaries', '2_source_debug', '3_traces', '4_model', '5_events'];
  for (const layer of required) {
    const present = layer in layers;
    results.push({ layer, present, data: present ? Object.keys(layers[layer]) : [] });
  }

  // Verify commitment (canonical: sorted keys, Python-compatible separators)
  const sortObj = (o) => {
    if (typeof o !== 'object' || o === null) return o;
    if (Array.isArray(o)) return o.map(sortObj);
    return Object.keys(o).sort().reduce((r, k) => { r[k] = sortObj(o[k]); return r; }, {});
  };
  // Python json.dumps(sort_keys=True) uses ", " and ": " separators
  const canonical = JSON.stringify(sortObj(layers)).replace(/,/g, ', ').replace(/:/g, ': ');
  const recomputed = sha256(canonical);
  const commitmentValid = recomputed === chain.commitment;

  // Verify trace hash if perf_data exists
  let traceValid = null;
  if (layers['3_traces'] && layers['3_traces'].perf_data) {
    try {
      const data = fs.readFileSync(layers['3_traces'].perf_data);
      const hash = sha256(data);
      traceValid = hash === layers['3_traces'].hash;
    } catch (e) {
      traceValid = false;
    }
  }

  // Verify source hash
  let sourceValid = null;
  if (layers['2_source_debug'] && layers['2_source_debug'].source_hash) {
    try {
      const data = fs.readFileSync(layers['2_source_debug'].source);
      sourceValid = sha256(data) === layers['2_source_debug'].source_hash;
    } catch (e) {
      sourceValid = false;
    }
  }

  return {
    chain_file: chainFile,
    timestamp: chain.timestamp,
    commitment: chain.commitment,
    commitment_valid: commitmentValid,
    trace_valid: traceValid,
    source_valid: sourceValid,
    layers: results,
    complete: results.every(r => r.present),
  };
}

function renderReport(report) {
  console.log('=== zkPerf Chain Verification ===');
  console.log(`File:       ${report.chain_file}`);
  console.log(`Timestamp:  ${report.timestamp}`);
  console.log(`Commitment: ${report.commitment}`);
  console.log(`Valid:      ${report.commitment_valid ? '✅' : '❌'}`);
  console.log(`Complete:   ${report.complete ? '✅ all 5 layers' : '❌ missing layers'}`);
  if (report.trace_valid !== null)
    console.log(`Trace:      ${report.trace_valid ? '✅' : '❌'}`);
  if (report.source_valid !== null)
    console.log(`Source:     ${report.source_valid ? '✅' : '❌'}`);
  console.log('\nLayers:');
  for (const l of report.layers) {
    console.log(`  ${l.present ? '✅' : '❌'} ${l.layer}: ${l.data.join(', ')}`);
  }
}

// Main
const chainFile = process.argv[2] || 'proofs/witness-chain.json';
if (!fs.existsSync(chainFile)) {
  console.error(`Chain file not found: ${chainFile}`);
  console.error('Run: python3 examples/zkperf-chain.py');
  process.exit(1);
}

const report = verifyChain(chainFile);
renderReport(report);

// Save verification
fs.writeFileSync('proofs/verification.json', JSON.stringify(report, null, 2));
console.log('\nSaved: proofs/verification.json');
