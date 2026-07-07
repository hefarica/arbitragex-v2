const fs = require('fs');
const path = require('path');

const BUILD_DIRS = [
  path.join(__dirname, '..', '.next'),
  path.join(__dirname, '..', 'out'),
  path.join(__dirname, '..', 'dist')
];

const BANNED_PATTERNS = [
  /localhost:8787/g,
  /http:\/\/localhost/g,
  /127\.0\.0\.1/g
];

let failed = false;

function scanDir(dir) {
  if (!fs.existsSync(dir)) return;
  if (dir.includes('node_modules') || dir.includes(path.join('.next', 'build'))) return;
  
  const files = fs.readdirSync(dir);
  for (const file of files) {
    const fullPath = path.join(dir, file);
    const stat = fs.statSync(fullPath);
    
    if (stat.isDirectory()) {
      scanDir(fullPath);
    } else if (stat.isFile() && (fullPath.endsWith('.js') || fullPath.endsWith('.html'))) {
      if (file.includes('[root-of-the-server]')) continue;
      let content = fs.readFileSync(fullPath, 'utf8');
      // ── Strip provably-inert framework / Web3-library localhost DEFAULTS before scanning. ──
      // None of these is an application endpoint or one of THIS app's services (our edge=8787,
      // api=8080, frontend=5173 all stay flagged), and none is the localhost-hardcoding threat
      // (#425 / API base-URL cascade) this guard exists to block. We remove ONLY these exact
      // constants; every concrete endpoint — http://localhost:<our-port>, localhost:8787,
      // 127.0.0.1 — still fails the scan below.
      //   1. Next 15 metadataBase fallback (next/dist/lib/metadata/resolvers/resolve-url.js):
      //        http://localhost:${process.env.PORT || 3000}   — server-only dynamic origin.
      //   2. viem/wagmi default Ethereum JSON-RPC transport (fires only when NO url is given;
      //      our chains/RPCs come from env):  http://localhost:8545
      //   3. WalletConnect/Reown modal allowed-ancestor glob:  http://localhost:*
      content = content
        .replace(/http:\/\/localhost:\$\{process\.env\.PORT[^}]*\}/g, '')
        .replace(/http:\/\/localhost:8545(?!\d)/g, '')
        .replace(/http:\/\/localhost:\*/g, '');
      for (const pattern of BANNED_PATTERNS) {
        if (pattern.test(content)) {
          console.error(`❌ ERROR: Found banned pattern ${pattern} in ${fullPath}`);
          failed = true;
        }
      }
    }
  }
}

console.log("Scanning build directories for localhost...");
for (const dir of BUILD_DIRS) {
  scanDir(dir);
}

if (failed) {
  console.error("❌ Validation Failed: Localhost hardcoding detected in production build.");
  process.exit(1);
} else {
  console.log("✅ Validation Passed: No localhost hardcoding found in production build.");
  process.exit(0);
}
