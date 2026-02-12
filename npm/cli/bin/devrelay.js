#!/usr/bin/env node

const { execFileSync } = require("child_process");
const path = require("path");

const platform = process.platform;
const arch = process.arch;

// Map Node platform/arch to our package names (darwin, linux only)
const platformMap = {
  darwin: { arm64: "darwin-arm64", x64: "darwin-x64" },
  linux: { arm64: "linux-arm64", x64: "linux-x64" },
};

const pkgName = platformMap[platform]?.[arch]
  ? `@devrelay/${platformMap[platform][arch]}`
  : null;

if (!pkgName) {
  console.error(
    `devrelay does not support ${platform}/${arch}. Supported: darwin (arm64, x64), linux (arm64, x64).`
  );
  process.exit(1);
}

let binaryPath;
try {
  const pkgJsonPath = require.resolve(`${pkgName}/package.json`);
  const pkgDir = path.dirname(pkgJsonPath);
  binaryPath = path.join(pkgDir, "bin", "devrelay");
} catch (err) {
  if (err.code === "MODULE_NOT_FOUND") {
    console.error(
      `The platform-specific package ${pkgName} is not installed. Try reinstalling @devrelay/cli.`
    );
    process.exit(1);
  }
  throw err;
}

const args = process.argv.slice(2);
try {
  execFileSync(binaryPath, args, {
    stdio: "inherit",
  });
} catch (err) {
  process.exit(err.status ?? 1);
}
