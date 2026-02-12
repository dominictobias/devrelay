# Development

## Prerequisites

- **Rust** 1.70+ (for building the binary)
- **Node.js** 14+ (for the npm wrapper and version script)
- Access to the **@devrelay** npm org (to publish packages)
- **npm publish** from CI uses **Trusted Publishing** (OIDC) once configured. See **First-time npm setup** below.

### First-time npm setup

npm does not let you create packages in the UI—the first publish creates them. Do this once:

1. **First publish (creates the 5 packages)**  
   Create a granular access token on [npm → Access Tokens](https://www.npmjs.com/) with **“Bypass two-factor authentication”** checked and **Read and write** for the scope/packages you need. Add it as the **NPM_TOKEN** repo secret. Run the **Publish NPM** workflow once (e.g. via a release or workflow_dispatch). That publish creates `@devrelay/darwin-arm64`, `@devrelay/darwin-x64`, `@devrelay/linux-x64`, `@devrelay/linux-arm64`, and `@devrelay/cli` on npm.

2. **Switch to Trusted Publishing**  
   On npm, for **each** of those five packages, open Package → **Settings** → **Trusted Publisher** → **GitHub Actions**, and set your org/user, repository name, and workflow filename **`publish-npm.yml`**. Save.

3. **Drop the token**  
   Remove the **NPM_TOKEN** secret from the repo. Future runs use OIDC only (no token, no “Bypass 2FA” needed).

## Building locally

```bash
cargo build --release
./target/release/devrelay
```

To build for a specific target (e.g. for testing cross-compilation):

```bash
rustup target add x86_64-apple-darwin   # example
cargo build --release --target x86_64-apple-darwin
```

## Releasing to NPM

Releases are published automatically when you **create a GitHub Release**. The tag version must match the version in the repo.

### 1. Bump the version

Run the bump script so `Cargo.toml` and `npm/cli/package.json` (and its `optionalDependencies`) stay in sync:

```bash
./scripts/bump-version.sh <version>
```

Example:

```bash
./scripts/bump-version.sh 0.2.0
```

### 2. Commit and push

```bash
git add Cargo.toml npm/cli/package.json
git commit -m "Release v0.2.0"
git push
```

### 3. Create a GitHub Release

1. Open **Releases** in the repo on GitHub.
2. Click **Draft a new release**.
3. Choose **Tag**: create a new tag `v<version>` (e.g. `v0.2.0`). The tag must match the version you bumped (with a `v` prefix).
4. Set the release title (e.g. `v0.2.0`) and add any release notes.
5. Click **Publish release**.

Publishing the release triggers the **Publish NPM** workflow (`.github/workflows/publish-npm.yml`). It will:

1. Build the Rust binary for all four platforms (darwin arm64/x64, linux arm64/x64).
2. Publish the platform packages: `@devrelay/darwin-arm64`, `@devrelay/darwin-x64`, `@devrelay/linux-x64`, `@devrelay/linux-arm64`.
3. Publish the main package **@devrelay/cli** (version is taken from the release tag).

Check the **Actions** tab for workflow status. After it completes, the new version will be available on npm.

### Summary

| Step | Action |
|------|--------|
| 1 | `./scripts/bump-version.sh X.Y.Z` |
| 2 | Commit and push the version bump |
| 3 | Create a GitHub Release with tag `vX.Y.Z` and publish it |

The tag **must** be `v` + the version you bumped (e.g. `v0.2.0`). The workflow reads the version from the tag and uses it for all published packages.
