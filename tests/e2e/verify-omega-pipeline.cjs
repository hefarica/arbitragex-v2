#!/usr/bin/env node
/**
 * OMEGA Pipeline VPS Verification Script
 * Simple Node.js script to verify the hot-path pipeline endpoints
 */

const http = require('http');

const EDGE_URL = process.env.ARBX_EDGE_URL || 'http://195.201.235.70:8787';
const WS_URL = process.env.ARBX_WS_URL || 'http://195.201.235.70:8080';

function fetch(url) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => resolve({ status: res.statusCode, data }));
    });
    req.on('error', reject);
    req.setTimeout(5000, () => reject(new Error('Timeout')));
  });
}

async function main() {
  console.log('🔍 OMEGA Pipeline VPS Verification\n');
  console.log(`Edge URL: ${EDGE_URL}`);
  console.log(`WS URL: ${WS_URL}\n`);

  let passed = 0;
  let failed = 0;

  // Test 1: Hot Health Endpoint
  try {
    const start = Date.now();
    const res = await fetch(`${EDGE_URL}/hot/v1/health/fast`);
    const latency = Date.now() - start;
    const json = JSON.parse(res.data);

    if (json.status === 'healthy' && latency < 100) {
      console.log(`✅ Hot Health: ${json.status} (${latency}ms)`);
      passed++;
    } else {
      console.log(`❌ Hot Health: unexpected response`);
      failed++;
    }
  } catch (e) {
    console.log(`❌ Hot Health: ${e.message}`);
    failed++;
  }

  // Test 2: Detected Opportunities Endpoint
  try {
    const res = await fetch(`${EDGE_URL}/hot/v1/opportunities/detected?count=5`);
    const json = JSON.parse(res.data);

    if (json.stream === 'arbx:hot:detected' && Array.isArray(json.opportunities)) {
      console.log(`✅ Detected Endpoint: ${json.opportunities.length} opportunities`);
      passed++;
    } else {
      console.log(`❌ Detected Endpoint: unexpected response`);
      failed++;
    }
  } catch (e) {
    console.log(`❌ Detected Endpoint: ${e.message}`);
    failed++;
  }

  // Test 3: Throughput Metrics Endpoint
  try {
    const res = await fetch(`${EDGE_URL}/hot/v1/metrics/throughput`);
    const json = JSON.parse(res.data);

    if (json.throughput && typeof json.latency_ms === 'number') {
      console.log(`✅ Metrics Endpoint: detected=${json.throughput.detected}, latency=${json.latency_ms}ms`);
      passed++;
    } else {
      console.log(`❌ Metrics Endpoint: unexpected response`);
      failed++;
    }
  } catch (e) {
    console.log(`❌ Metrics Endpoint: ${e.message}`);
    failed++;
  }

  // Test 4: API Server Health
  try {
    const res = await fetch(`${WS_URL}/api/v1/health`);
    const json = JSON.parse(res.data);

    if (json.system_status === 'healthy' && json.math_guardian === 'passed') {
      console.log(`✅ API Health: ${json.system_status}, entropy=${json.entropy}`);
      passed++;
    } else {
      console.log(`❌ API Health: unexpected response`);
      failed++;
    }
  } catch (e) {
    console.log(`❌ API Health: ${e.message}`);
    failed++;
  }

  console.log(`\n📊 Results: ${passed} passed, ${failed} failed`);
  process.exit(failed > 0 ? 1 : 0);
}

main();
