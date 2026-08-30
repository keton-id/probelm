# midas-model-test (Rust CLI)

CLI untuk **testing provider / probe model** di gateway **9Router** — ping,
latency (TTFT), dan capabilities. Output tabel di terminal (manusia) atau JSON
(agent). Dibangun ulang dari `midas-model-tests` (bash) dalam **Rust**; skema
config tetap kompatibel sehingga drop-in.

Requirement: [`srd-probelm/midas-model-test-requirement.md`](../srd-probelm/midas-model-test-requirement.md)

## Build

```bash
cargo build --release          # binary: target/release/mtest
```

Requirement: Rust (edisi 2021). HTTP: `reqwest` (tokio), CLI: `clap`,
JSON: `serde`.

## Config

Prioritas **env > file** (file default `config.json`; ganti dengan `--config`):

| Value | Env | JSON field | Default |
|-------|-----|-----------|---------|
| baseUrl | `ROUTER_URL` | `endpoint.baseUrl` | `http://localhost:20128` |
| apiKey | `ROUTER_KEY` | `endpoint.apiKey` | — (wajib) |
| models | | `models[]` | dari discovery |
| prompt | | `defaultPrompt` / `prompts.<model>` | `"Reply with exactly: OK"` |
| maxTokens / temperature / timeoutSeconds | | | 64 / 0 / 120 |

```json
{
  "endpoint": { "baseUrl": "http://localhost:20128", "apiKey": "sk-..." },
  "models": ["midas/glm-4.7", "midas/deepseek-v4-pro"],
  "defaultPrompt": "Reply with exactly: OK",
  "maxTokens": 64, "temperature": 0, "timeoutSeconds": 120
}
```

## Usage

```bash
# Discover models (tabel / JSON)
mtest list --config config.json
mtest list --config config.json --json
mtest list --config config.json --owned-by midas
mtest list --config config.json --prefix midas --list-models   # ekspor models.json

# Probe model (ping + latency + caps default)
mtest probe --config config.json --model midas/glm-4.7
mtest probe --config config.json --model a,b --json           # untuk agent
mtest probe --config config.json --prefix midas --cap reasoning
mtest probe --config config.json --ping | --latency | --caps-only
mtest probe --config config.json --jobs 4                     # paralel
```

## Output

**Tabel (default):** kolom `MODEL PING TTFT(s) TOTAL(s) TOK/s CAPABILITIES`.

**JSON (`--json`):** satu objek stabil untuk agent:

```json
{ "probes": [ {
    "model": "midas/glm-4.7",
    "ping":  { "ok": true, "http_code": 200 },
    "latency": { "ttft_secs": 0.08, "total_secs": 0.12, "tokens": 1, "rate_per_sec": 7.9 },
    "caps": { "vision": false, "reasoning": true, "contextWindow": 200000, "maxOutput": 128000, ... }
} ] }
```

Field absen bila test di-skip.

**Exit code:** `0` sukses / semua OK · `1` ada model ping gagal · `2` error config/usage.

## Note keamanan

- Hanya memakai **gateway key** (untuk auth ke 9Router) — tidak ada key upstream
  provider di tool ini.
- `ROUTER_KEY` env override file; jangan commit `config.json`.
- API key tidak pernah dicetak.

## License

MIT.
