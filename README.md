# probelm / mtest

CLI diagnostik, benchmarking, dan healthcheck model LLM di gateway **9Router** — mengukur ketersediaan (ping), latensi (*Time-to-First-Token* / TTFT), throughput (*tokens/sec*), dan inspeksi kapabilitas model.

Mendukung output tabel berwarna interaktif, format tabel Markdown (siap tempel di PR/issue), serta JSON terstruktur untuk otomasi AI Agent.

---

## ⚡ 30-Second Quickstart

```bash
# 1. Clone & install ke $HOME/.local/bin
git clone https://github.com/resincode/probelm.git
cd probelm
./install.sh

# 2. Inisialisasi konfigurasi (otomatis mendeteksi 9Router lokal & API key)
mtest init --global -y

# 3. Jalankan pengujian model
mtest test
```

*Catatan: Anda bisa memanggil binary menggunakan perintah `mtest` atau alias `probelm`.*

---

## 🚀 Panduan Penggunaan (*Usage & Recipes*)

Subcommand utama untuk pengujian adalah **`test`** (dengan alias **`check`**, **`bench`**, **`probe`**, atau **`run`**).

### 1. Pengujian Model Sederhana & Positional Arguments
```bash
# Uji model yang ada di file konfigurasi
mtest test

# Uji satu atau beberapa model tertentu secara langsung (positional)
mtest test midas/glm-5.2
mtest test midas/glm-5.2 midas/deepseek-v4-pro

# Uji SEMUA model yang terdaftar di 9Router (bypass list config)
mtest test --all
```

### 2. Filter Berdasarkan Grup / Prefix & Kapabilitas
```bash
# Uji semua model dengan prefix tertentu (misal 'cx/' atau 'midas/')
mtest test --prefix cx
mtest test --prefix midas

# Uji model berdasarkan owner/grup (misal 'combo', 'midas', 'cx')
mtest test --owned-by combo

# Uji hanya model yang memiliki kapabilitas tertentu (reasoning, vision, tools)
mtest test --cap reasoning
mtest test --all --cap vision
```

### 3. Pengurutan Hasil (*Sorting*)
```bash
# Urutkan berdasarkan kecepatan generasi (TOK/s tertinggi)
mtest test --all --sort speed

# Urutkan berdasarkan latensi respons terendah (TTFT tercepat)
mtest test --all --sort ttft

# Urutkan secara alfabetis berdasarkan nama model
mtest test --all --sort name
```

### 4. Custom Prompt & Stress Testing Paralel
```bash
# Menguji model dengan prompt kustom dari terminal
mtest test midas/deepseek-v4-pro -p "Jelaskan konsep zero-copy dalam 2 kalimat."

# Menjalankan pengujian 8 model secara bersamaan (paralel)
mtest test --all --jobs 8
```

### 5. Pilihan Format Output
```bash
# Format tabel terminal berwarna (default)
mtest test

# Format tabel Markdown (siap salin ke GitHub PR / issue / docs)
mtest test midas/glm-5.2 midas/deepseek-v4-pro --md

# Format JSON terstruktur (ideal untuk pipeline / agent / jq)
mtest test --all --json

# Hanya cek ping / latensi / kapabilitas saja
mtest test --ping
mtest test --latency
mtest test --all --caps-only
```

### 6. Model Discovery (`list`)
```bash
# Menampilkan semua model yang tersedia di gateway
mtest list

# Filter model list
mtest list --prefix cx
mtest list --owned-by midas

# Ekspor katalog model ke file JSON
mtest list --prefix midas --list-models midas-models.json
```

---

## 🖥️ Contoh Output Tampilan

### Format Terminal Interaktif:
```text
MODEL                                PING   TTFT(s) TOTAL(s)   TOK/s  CAPS            CTX    OUT
-----------------------------------------------------------------------------------------------
midas/deepseek-v4-pro                OK       0.101    0.106    12.5  🧠 🛠            1m   384k
midas/glm-5.2                        OK       1.731    1.838    16.9  🧠 🛠          200k   128k
cx/gpt-5.6-sol                       OK       0.842    0.855    21.4  🧠 👁 🛠 🔍    372k   128k
-----------------------------------------------------------------------------------------------
Legend: 🧠 Reasoning  👁 Vision  🛠 Tools  📄 PDF  🔍 Search  🎙 Audio  🎬 Video  🎨 Image
```

### Format Markdown Table (`--md`):
| Model | Ping | TTFT (s) | Total (s) | Tok/s | Caps | Context | Max Out |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `midas/glm-5.2` | OK | 1.731 | 1.838 | 16.9 | 🧠 🛠 | 200k | 128k |
| `midas/deepseek-v4-pro` | OK | 0.101 | 0.106 | 12.5 | 🧠 🛠 | 1m | 384k |

---

## ⚙️ Konfigurasi

Prioritas resolusi konfigurasi:
1. Argument CLI `--config <path>`
2. Environment Variables (`ROUTER_KEY` & `ROUTER_URL`)
3. File lokal `./config.json`
4. File global `$HOME/.config/probelm/config.json`

Contoh isi `config.json`:
```json
{
  "endpoint": {
    "baseUrl": "http://localhost:20128",
    "apiKey": "sk-..."
  },
  "models": [
    "midas/glm-5.2",
    "midas/deepseek-v4-pro"
  ],
  "defaultPrompt": "Reply with exactly: OK",
  "maxTokens": 64,
  "temperature": 0.0,
  "timeoutSeconds": 120
}
```

---

## 📦 Instalasi & Build Manual

### Menggunakan Installer Script (Rekomendasi macOS / Linux):
```bash
./install.sh

# Untuk uninstall:
./install.sh --uninstall
```

### Build Manual dengan Cargo:
```bash
cargo build --release
# Binary berada di: target/release/mtest
```

---

## 🛡️ Keamanan
- Hanya menggunakan **Gateway API Key** (untuk autentikasi ke 9Router lokal) — tidak ada API key upstream provider pihak ketiga yang disimpan oleh tool ini.
- API key tidak pernah dicetak utuh di log/terminal (*auto-masked* saat setup).

---

## 📄 Lisensi
MIT License.
