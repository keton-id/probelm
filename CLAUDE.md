# probelm

`probelm` is the public command and crate name. The scoped npm package is `@keton-id/probelm`. The repository is a Rust CLI for probing and benchmarking models through an OpenAI-compatible gateway.

For code changes, use `/forgeguard-engineering`.

## Naming and compatibility

- Primary command and binary: `probelm`.
- Crate: `probelm`.
- npm package: `@keton-id/probelm`.
- A legacy executable remains available for compatibility with existing users. Do not advertise it in new documentation, examples, installers, package metadata, or release notes.
- Both command names are built from the same `src/main.rs`; no behavior split is required.
- Keep command examples in `README.md`, `README_ID.md`, `npm/README.md`, and release documentation on `probelm`.

## Repository artifacts

- `README.md`: canonical English documentation.
- `README_ID.md`: Indonesian translation of the canonical user documentation.
- `npm/README.md`: npm package copy of `README.md`; update it in the same change.
- `script/install.sh`: macOS/Linux release installer, with checkout build fallback.
- `script/install.ps1`: Windows PowerShell release installer.
- `Makefile`: local formatting, checks, build, packaging, and installer targets.
- `npm/`: npm wrapper that downloads and verifies the matching GitHub Release archive.
- `src/mcp.rs`: stdio MCP server, tool schemas, and Kurir harness registration.
- `packaging/probelm.rb.tmpl`: Homebrew formula source.
- `packaging/probelm.json.tmpl`: Scoop manifest source.
- `.github/workflows/ci.yml`: standard Rust format, lint, check, and test gate.
- `.github/workflows/release-please.yml`: manually started Release Please preparation plus merged Release Please PR finalization. It does not run for ordinary commits to the default branch.
- `.github/workflows/release.yml`: builds six target archives, uploads checksums, and publishes distribution channels after a published GitHub Release. It also supports manual recovery for an existing tag.
- `.github/CODEOWNERS`: assigns `@keton-id/maintainers` as the default owner.
- `.github/PULL_REQUEST_TEMPLATE.md`: PR checklist.
- `.github/ISSUE_TEMPLATE/`: bug report, feature request, and configuration templates.
- `CHANGELOG.md`: Release Please changelog at repository root. `CHANEGLOG.md` in the original request is treated as a filename typo; keep the conventional spelling required by Release Please.
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, and `SECURITY.md`: public project policies.

## Release contract

Release Please owns the version. Do not bump `Cargo.toml` or create tags manually.

1. Merge conventional commits to the current default branch (`master` in this checkout; update workflow branch settings if the repository is renamed to `main`).
2. Start `.github/workflows/release-please.yml` with `workflow_dispatch` after batching changes.
3. Release Please opens or updates its release PR. Ordinary commits do not trigger release preparation.
4. Merge the Release Please PR. The workflow creates tag `vX.Y.Z` and a GitHub Release.
5. The published-release event starts `.github/workflows/release.yml`.
6. The release workflow builds exactly six archives:
   - `probelm-macos-aarch64.tar.gz`
   - `probelm-macos-x86_64.tar.gz`
   - `probelm-windows-aarch64.zip`
   - `probelm-windows-x86_64.zip`
   - `probelm-linux-aarch64.tar.gz`
   - `probelm-linux-x86_64.tar.gz`
7. The workflow uploads `SHA256SUMS` and one `<asset>.sha256` beside every archive before distribution jobs run.
8. Sibling distribution jobs publish the crate to crates.io through OIDC, publish npm package `@keton-id/probelm` through npm Trusted Publishing/OIDC, and update `keton-id/homebrew-tap` plus `keton-id/scoop-bucket`.

Required GitHub configuration:

- `RELEASE_PLEASE_TOKEN`: token allowing Release Please to open/update its PR and create the release.
- `TAP_PUBLISH_TOKEN`: write access to `keton-id/homebrew-tap` and `keton-id/scoop-bucket`.
- GitHub environment `Release` trusted by npm package `@keton-id/probelm` and crates.io crate `probelm`.
- npm Trusted Publisher configured for this repository and `.github/workflows/release.yml`.
- crates.io Trusted Publisher configured for this repository and `.github/workflows/release.yml`.

No npm or crates.io long-lived token belongs in repository files.

## Release asset consumers

Asset names are a compatibility contract shared by `.github/workflows/release.yml`, `script/install.sh`, `script/install.ps1`, `npm/scripts/install.js`, the Homebrew template, and the Scoop template. Change all consumers together.
- Every downloader verifies the archive SHA-256 before extraction. The installer creates `probelm` as the primary command and preserves a local compatibility alias for existing users.

## MCP contract

- `probelm mcp serve` is the harness entrypoint and uses RMCP stdio transport.
- `probelm mcp install` requires an interactive stdin/stdout TTY when `--client` is omitted, matching the FluxGuard installer pattern.
- `probelm mcp install --client <harness>` is the non-interactive path for automation.
- Kurir owns harness-specific registration, configuration paths, scopes, backups, and delegated client CLIs.
- The server exposes `list_models`, `probe_models`, and `sync_specs` with JSON-schema-backed responses.
- Gateway credentials stay in runtime configuration and are never returned by MCP responses.

## Local checks

```bash
make fmt
make lint
make test
make check-all
make package
make npm-pack
```

Keep API keys and personal data out of source, tests, commits, issues, and release metadata.
