#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>"
  echo "Example: $0 0.2.0"
  exit 1
fi

VERSION="$1"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Cargo.toml
sed -i.bak "s/^version = .*/version = \"$VERSION\"/" "$ROOT/Cargo.toml"
rm -f "$ROOT/Cargo.toml.bak"

# npm/cli/package.json - version and optionalDependencies
FILE="$ROOT/npm/cli/package.json" VERSION="$VERSION" node -e '
const fs = require("fs");
const j = JSON.parse(fs.readFileSync(process.env.FILE, "utf8"));
j.version = process.env.VERSION;
for (const k of Object.keys(j.optionalDependencies || {})) {
  j.optionalDependencies[k] = process.env.VERSION;
}
fs.writeFileSync(process.env.FILE, JSON.stringify(j, null, 2));
'

echo "Bumped version to $VERSION in Cargo.toml and npm/cli/package.json"
