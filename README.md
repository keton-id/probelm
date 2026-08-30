# midas-model-test (Rust CLI)

CLI untuk **testing provider / probe model** di gateway **9Router** — ping,
latency (TTFT), dan capabilities. Output tabel di terminal (manusia) atau JSON
(agent). Dibangun ulang dari `midas-model-tests` (bash) dalam **Rust**; skema
config tetap kompatibel sehingga drop-in.

Requirement: [`srd-probelm/midas-model-test-requirement.md`](../srd-probelm/midas-model-test-requirement.md)

## Install & Build

### Cara 1: Installer Script (Rekomendasi di macOS / Linux)

```bash
./install.sh
```
*Script ini akan otomatis meng-compile binary release dan memasang `mtest` serta alias `probelm` ke `$HOME/.local/bin`.*

Untuk uninstall:
```bash
./install.sh --uninstall
```

### Cara 2: Manual Cargo Build

```bash
cargo build --release          # binary: target/release/mtest
```
Requirement: Rust (edisi 2021). HTTP: `reqwest` (tokio), CLI: `clap`,
JSON: `serde`.

## Quickstart & Init

Gunakan perintah `init` untuk membuat file konfigurasi secara interaktif (otomatis mendeteksi 9Router lokal dan API key):

```bash
# Interactive wizard (tanya URL, API key, filter model, target lokasi)
mtest init

# Simpan secara global (~/.config/probelm/config.json) agar bisa dipanggil dari folder mana saja
mtest init --global

# Non-interactive (gunakan nilai default & auto-detected)
mtest init -y
```

## Config

Prioritas resolusi konfigurasi:
1. `--config <path>`
2. Environment variables (`ROUTER_KEY` & `ROUTER_URL`)
3. File lokal `./config.json`
4. File global `$HOME/.config/probelm/config.json`

| Value | Env | JSON field | Default |
|-------|-----|-----------|---------|
| baseUrl | `ROUTER_URL` | `endpoint.baseUrl` | `http://localhost:20128` |
| apiKey | `ROUTER_KEY` | `endpoint.apiKey` | — (auto-detected dari 9Router) |
| models | | `models[]` | dari discovery |
| prompt | | `defaultPrompt` / `prompts.<model>` | `"Reply with exactly: OK"` |
| maxTokens / temperature / timeoutSeconds | | | 64 / 0 / 120 |

```json
{
  "endpoint": { "baseUrl": "http://localhost:20128", "apiKey": "sk-..." },
  "models": ["midas/glm-5.2", "midas/deepseek-v4-pro"],
  "defaultPrompt": "Reply with exactly: OK",
  "maxTokens": 64, "temperature": 0, "timeoutSeconds": 120
}
```
## Usage

```bash
# Discover models (tabel / JSON)
mtest list
mtest list --json
mtest list --owned-by midas
mtest list --prefix cx
mtest list --prefix midas --list-models midas-models.json   # ekspor models.json

# Probe model (ping + latency + caps default)
mtest probe                                    # probe model yang ada di config.json
mtest probe --all                              # probe SEMUA model di 9Router (bypass config)
mtest probe --prefix cx                        # probe semua model berawalan 'cx/'
mtest probe --owned-by combo                   # probe berdasarkan owner/combo
mtest probe --model midas/glm-5.2,cx/gpt-5.5   # probe model spesifik
mtest probe --cap reasoning                    # probe hanya model reasoning
mtest probe --ping | --latency | --caps-only   # pilih mode tes
mtest probe --all --jobs 8                     # jalankan 8 worker paralel
```
## Output

**Tabel (default):** kolom `MODEL PING TTFT(s) TOTAL(s) TOK/s CAPS CTX OUT` dilengkapi ikon dan legenda.

```text
MODEL                                PING   TTFT(s) TOTAL(s)   TOK/s  CAPS            CTX    OUT
-----------------------------------------------------------------------------------------------
midas/glm-5.2                        OK       1.731    1.838    16.9  🧠 🛠          200k   128k
midas/deepseek-v4-pro                OK       0.101    0.106     9.5  🧠 🛠            1m   384k
cx/gpt-5.6-sol                       -            -        -       -  🧠 👁 🛠 🔍    372k   128k
-----------------------------------------------------------------------------------------------
Legend: 🧠 Reasoning  👁 Vision  🛠 Tools  📄 PDF  🔍 Search  🎙 Audio  🎬 Video  🎨 Image
```
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
