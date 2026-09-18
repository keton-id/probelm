# probelm

`probelm` is a Rust CLI for probing and benchmarking models through an OpenAI-compatible gateway. It measures availability, time to first token (TTFT), total latency, throughput, and model capabilities.

## Quickstart

### macOS and Linux

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/keton-id/probelm/master/script/install.sh | bash
probelm init --global -y
probelm test
```

To install from a checkout instead:

```bash
./script/install.sh
```

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/keton-id/probelm/master/script/install.ps1 | iex
probelm init --global -y
probelm test
```

The installers select the published binary for the current operating system and architecture. `PROBELM_VERSION` or the PowerShell `-Version` parameter can pin a release.

## Commands

```text
probelm init [options]
probelm list [options]
probelm test [models] [options]
probelm sync-specs
probelm tui [options]
probelm config [options]
```

`test` also accepts the aliases `probe`, `check`, `bench`, and `run`.
`tui` also accepts the aliases `manage` and `dashboard`.

## MCP for harnesses

`probelm` can run as a local MCP server over stdio. Register it with an
agent harness through Kurir:

```bash
# Interactive TTY installer
probelm mcp install

# Non-interactive registration
probelm mcp install --client claude-code
probelm mcp install --client codex --project

# The command launched by the harness
probelm mcp serve
```

The interactive installer requires a real TTY so it can present the harness
selection menu. Pass `--client` in automation. Registration writes a stdio
entry that launches `probelm mcp serve`; the server reads the same gateway
configuration used by the CLI and keeps protocol output on stdout.

### Initialize configuration

```bash
probelm init --global -y
probelm init --url http://localhost:20128 --api-key sk-...
```

Configuration resolution order:

1. `--config <path>`
2. `ROUTER_KEY` and `ROUTER_URL`
3. `./config.json`
4. `$HOME/.config/probelm/config.json`

Example configuration:

```json
{
  "endpoint": {
    "baseUrl": "http://localhost:20128",
    "apiKey": "sk-..."
  },
  "models": ["midas/glm-5.2", "midas/deepseek-v4-pro"],
  "defaultPrompt": "Reply with exactly: OK",
  "maxTokens": 64,
  "temperature": 0.0,
  "timeoutSeconds": 120
}
```

### Probe models

```bash
# Models from configuration
probelm test

# One or more explicit models
probelm test midas/glm-5.2 midas/deepseek-v4-pro

# Every model exposed by the gateway
probelm test --all

# Wildcards, prefixes, ownership, and capabilities
probelm test 'cx/*'
probelm test --prefix cx,ag,midas
probelm test --owned-by combo
probelm test --all --cap reasoning

# Custom prompt and parallel jobs
probelm test midas/deepseek-v4-pro --prompt 'Reply in two sentences.'
probelm test --all --jobs 8
```

### Sort and format results

```bash
probelm test --all --sort ctx,speed
probelm test --all --sort ttft:asc
probelm test --all --md
probelm test --all --json
probelm test --ping
probelm test --latency
probelm test --all --caps-only
```

Sort keys include `ctx`/`context`, `speed`/`rate`, `ttft`/`latency`, `out`/`max_out`, `ping`/`status`, and `name`/`model`.

### Discover and synchronize models

```bash
probelm list
probelm list --prefix cx --owned-by midas
probelm list --prefix midas --list-models midas-models.json
probelm sync-specs
```

`sync-specs` downloads the authoritative model specification database from LiteLLM and updates local metadata used by the probe.

## Building from source

```bash
cargo build --release --locked
cargo run --release -- test --help
make check-all
```

The release build produces `probelm` as the primary binary.

## Distribution

Published releases provide six archives:

- macOS Apple silicon: `probelm-macos-aarch64.tar.gz`
- macOS Intel: `probelm-macos-x86_64.tar.gz`
- Windows ARM64: `probelm-windows-aarch64.zip`
- Windows x86-64: `probelm-windows-x86_64.zip`
- Linux ARM64: `probelm-linux-aarch64.tar.gz`
- Linux x86-64: `probelm-linux-x86_64.tar.gz`

Each archive has a matching `.sha256` file and is listed in `SHA256SUMS`. Homebrew, Scoop, npm, and the PowerShell installer consume these same assets.

## Security

Do not commit gateway API keys. Use environment variables or a configuration file excluded by `.gitignore`. Report vulnerabilities privately through the process in [SECURITY.md](SECURITY.md).

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), and [CLAUDE.md](CLAUDE.md) before opening a change.

## License

MIT. See the package metadata and repository distribution terms.
