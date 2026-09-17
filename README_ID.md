# probelm

`probelm` adalah CLI Rust untuk melakukan probe dan benchmarking model melalui gateway yang kompatibel dengan OpenAI. Tool ini mengukur ketersediaan, *time to first token* (TTFT), latensi total, throughput, dan kapabilitas model.

## Quickstart

### macOS dan Linux

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/keton-id/probelm/master/script/install.sh | bash
probelm init --global -y
probelm test
```

Instalasi dari checkout:

```bash
./script/install.sh
```

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/keton-id/probelm/master/script/install.ps1 | iex
probelm init --global -y
probelm test
```

Installer memilih binary yang sesuai dengan sistem operasi dan arsitektur. Versi dapat dipatok menggunakan `PROBELM_VERSION` atau parameter PowerShell `-Version`.

## Perintah

```text
probelm init [options]
probelm list [options]
probelm test [models] [options]
probelm sync-specs
```

`test` juga menerima alias `probe`, `check`, `bench`, dan `run`.

## MCP untuk harness

`probelm` dapat berjalan sebagai server MCP lokal melalui stdio. Daftarkan
ke harness agent menggunakan Kurir:

```bash
# Installer interaktif melalui TTY
probelm mcp install

# Registrasi non-interaktif
probelm mcp install --client claude-code
probelm mcp install --client codex --project

# Perintah yang dijalankan harness
probelm mcp serve
```

Installer interaktif membutuhkan TTY agar dapat menampilkan menu pemilihan
harness. Gunakan `--client` untuk automation. Registrasi membuat entry stdio
yang menjalankan `probelm mcp serve`; server memakai konfigurasi gateway yang
sama dengan CLI dan menjaga output protokol di stdout.

### Inisialisasi konfigurasi

```bash
probelm init --global -y
probelm init --url http://localhost:20128 --api-key sk-...
```

Urutan resolusi konfigurasi:

1. `--config <path>`
2. `ROUTER_KEY` dan `ROUTER_URL`
3. `./config.json`
4. `$HOME/.config/probelm/config.json`

Contoh konfigurasi:

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

### Probe model

```bash
# Model dari konfigurasi
probelm test

# Satu atau beberapa model tertentu
probelm test midas/glm-5.2 midas/deepseek-v4-pro

# Semua model yang tersedia di gateway
probelm test --all

# Wildcard, prefix, owner, dan kapabilitas
probelm test 'cx/*'
probelm test --prefix cx,ag,midas
probelm test --owned-by combo
probelm test --all --cap reasoning

# Prompt kustom dan job paralel
probelm test midas/deepseek-v4-pro --prompt 'Reply in two sentences.'
probelm test --all --jobs 8
```

### Mengurutkan dan memformat hasil

```bash
probelm test --all --sort ctx,speed
probelm test --all --sort ttft:asc
probelm test --all --md
probelm test --all --json
probelm test --ping
probelm test --latency
probelm test --all --caps-only
```

Kunci sort mencakup `ctx`/`context`, `speed`/`rate`, `ttft`/`latency`, `out`/`max_out`, `ping`/`status`, dan `name`/`model`.

### Menemukan dan menyinkronkan model

```bash
probelm list
probelm list --prefix cx --owned-by midas
probelm list --prefix midas --list-models midas-models.json
probelm sync-specs
```

`sync-specs` mengunduh database spesifikasi model resmi dari LiteLLM dan memperbarui metadata lokal yang digunakan probe.

## Build dari source

```bash
cargo build --release --locked
cargo run --release -- test --help
make check-all
```

Build release menghasilkan `probelm` sebagai binary utama.

## Distribusi

Setiap release menyediakan enam arsip:

- macOS Apple silicon: `probelm-macos-aarch64.tar.gz`
- macOS Intel: `probelm-macos-x86_64.tar.gz`
- Windows ARM64: `probelm-windows-aarch64.zip`
- Windows x86-64: `probelm-windows-x86_64.zip`
- Linux ARM64: `probelm-linux-aarch64.tar.gz`
- Linux x86-64: `probelm-linux-x86_64.tar.gz`

Setiap arsip memiliki file `.sha256` dan tercantum di `SHA256SUMS`. Homebrew, Scoop, npm, dan installer PowerShell memakai asset yang sama.

## Keamanan

Jangan commit API key gateway. Gunakan environment variable atau file konfigurasi yang dikecualikan oleh `.gitignore`. Laporkan kerentanan secara privat mengikuti proses di [SECURITY.md](SECURITY.md).

## Kontribusi

Baca [CONTRIBUTING.md](CONTRIBUTING.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), dan [CLAUDE.md](CLAUDE.md) sebelum mengajukan perubahan.

## Lisensi

MIT. Lihat metadata package dan ketentuan distribusi repository.
