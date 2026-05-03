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
  
  const files = fs.readdirSync(dir);
  for (const file of files) {
    const fullPath = path.join(dir, file);
    const stat = fs.statSync(fullPath);
    
    if (stat.isDirectory()) {
      scanDir(fullPath);
    } else if (stat.isFile() && (fullPath.endsWith('.js') || fullPath.endsWith('.html'))) {
      const content = fs.readFileSync(fullPath, 'utf8');
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
