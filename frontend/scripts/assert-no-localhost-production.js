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
      // Strip Next.js framework-owned metadataBase fallback before scanning.
      // Next 15 bundles this server-only DYNAMIC fallback into .next/server/chunks:
      //
      //   http://localhost:${process.env.PORT || 3000}
      //
      // Source: next/dist/lib/metadata/resolvers/resolve-url.js (createLocalMetadataBase).
      // It is a runtime default origin — NOT an application endpoint, NOT browser-shipped
      // client code (it never appears in .next/static), and NOT the localhost-hardcoding
      // threat (#425 / API base-URL cascade) this guard exists to block. We remove ONLY this
      // exact dynamic template; every concrete endpoint — http://localhost:<port>,
      // localhost:8787, 127.0.0.1 — still fails the scan below.
      content = content.replace(/http:\/\/localhost:\$\{process\.env\.PORT[^}]*\}/g, '');
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
