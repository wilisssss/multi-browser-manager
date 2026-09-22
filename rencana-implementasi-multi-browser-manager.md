# Rencana Implementasi: Multi Browser Manager

## 1. Ringkasan Proyek

**Multi Browser Manager (MBM)** adalah desktop application ringan dan cepat untuk mengelola banyak instance browser berbasis Chromium (Google Chrome, Chromium, Brave, Edge) dari satu dashboard terpusat. Setiap profile browser memiliki `user-data-dir` sendiri sehingga cookie, session, local storage, cache, dan extension antar profile terisolasi penuh — tidak saling bercampur. Cocok dipakai untuk mengelola banyak akun independen: akun bisnis/klien, akun tim yang dikelola satu operator, environment testing multi-tenant, atau pemisahan konteks personal/kerja.

## 2. Tujuan & Ruang Lingkup

### Termasuk dalam scope
- CRUD profile browser dengan isolasi data penuh (per-profile `user-data-dir`)
- Launch & stop browser per profile, termasuk bulk action untuk banyak profile sekaligus
- Konfigurasi proxy per profile (HTTP/SOCKS5), termasuk proxy yang butuh autentikasi username/password
- Dashboard: pencarian, filter, tag/grouping, status running/stopped real-time
- Import/export konfigurasi profile (backup & restore)
- Cross-platform: Linux (prioritas utama), Windows, macOS

### Di luar scope
Fingerprint spoofing (canvas, WebGL, audio, font), penyamaran flag automation (`navigator.webdriver`), randomisasi timezone/locale untuk memalsukan identitas device, atau teknik lain yang dirancang khusus untuk mengelabui sistem deteksi platform — tidak dibahas di rencana ini. Isolasi data per-profile pada poin di atas sudah menjawab kebutuhan pemisahan akun secara teknis: setiap profile punya cookie/session/storage sendiri tanpa ada yang bocor ke profile lain.

## 3. Tech Stack

| Layer | Pilihan | Alasan |
|---|---|---|
| Bahasa inti | Rust | Performa tinggi, memory-safe, cocok untuk binary ringan sesuai requirement |
| App framework | Tauri v2 | Pakai webview native OS (bukan bundle Chromium seperti Electron) → binary jauh lebih kecil (~5-10MB) dan RAM idle jauh lebih rendah; genuinely cross-platform dari satu codebase |
| Frontend | React + Vite + TailwindCSS | Selaras dengan stack yang sudah dipakai di project xray-proxy-manager, ekosistem component matang, build cepat |
| Database | SQLite via `rusqlite` (embedded) | Tanpa server terpisah, cukup untuk ratusan–ribuan profile, query relasional untuk tag/grouping |
| Icon set | `lucide-react` | Ringan, konsisten, gaya minimal |

Dashboard didesain minimal dan purposeful — fokus ke grid/list profile dengan status yang jelas, tanpa ornamen atau fitur yang tidak esensial.

## 4. Arsitektur Sistem

```
┌──────────────────────────────────────────────────┐
│                 Frontend (React)                   │
│    Dashboard · Profile Form · Proxy Manager         │
└─────────────────────┬──────────────────────────────┘
                       │ Tauri IPC (invoke / event)
┌─────────────────────▼──────────────────────────────┐
│                Rust Core (src-tauri)                 │
│  ┌───────────┐  ┌────────────┐  ┌───────────┐        │
│  │  Profile  │  │  Process   │  │   Proxy   │        │
│  │  Manager  │  │Orchestrator│  │  Manager  │        │
│  └─────┬─────┘  └─────┬──────┘  └─────┬─────┘        │
│        └──────────────┼───────────────┘               │
│                 ┌──────▼──────┐                        │
│                 │  DB Layer   │                        │
│                 │ (rusqlite)  │                        │
│                 └──────┬──────┘                        │
└────────────────────────┼────────────────────────────────┘
                          │
        ┌─────────────────▼─────────────────┐
        │  SQLite file + folder user-data-dir  │
        │        per profile (filesystem)       │
        └─────────────────┬─────────────────────┘
                           │
        ┌──────────────────▼──────────────────┐
        │   Proses OS: chrome/chromium/brave     │
        │   --user-data-dir=<path>               │
        │   --proxy-server=<...> (opsional)      │
        └───────────────────────────────────────────┘
```

## 5. Struktur Project

```
multi-browser-manager/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   ├── profile.rs      # command CRUD profile
│   │   │   ├── process.rs      # command launch/stop
│   │   │   ├── proxy.rs        # command CRUD proxy + test
│   │   │   └── backup.rs       # command export/import
│   │   ├── db/
│   │   │   ├── mod.rs
│   │   │   ├── schema.rs
│   │   │   └── migrations/
│   │   ├── browser/
│   │   │   ├── mod.rs
│   │   │   ├── detector.rs     # deteksi browser terinstall per OS
│   │   │   └── launcher.rs     # spawn & kill process
│   │   ├── models/
│   │   │   ├── mod.rs
│   │   │   ├── profile.rs
│   │   │   └── proxy.rs
│   │   └── error.rs
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── icons/
├── src/
│   ├── components/
│   │   ├── ProfileCard.tsx
│   │   ├── ProfileList.tsx
│   │   ├── ProfileForm.tsx
│   │   ├── ProxyManager.tsx
│   │   └── Dashboard.tsx
│   ├── hooks/
│   │   ├── useProfiles.ts
│   │   └── useProxies.ts
│   ├── lib/
│   │   └── tauri-api.ts
│   ├── App.tsx
│   └── main.tsx
├── package.json
└── vite.config.ts
```

## 6. Skema Database

```sql
CREATE TABLE profiles (
    id TEXT PRIMARY KEY,              -- UUID v4
    name TEXT NOT NULL,
    browser_type TEXT NOT NULL,       -- chrome | chromium | brave | edge
    user_data_dir TEXT NOT NULL UNIQUE,
    proxy_id TEXT REFERENCES proxies(id),
    notes TEXT,
    status TEXT NOT NULL DEFAULT 'stopped', -- stopped | running
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_used_at INTEGER
);

CREATE TABLE proxies (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    protocol TEXT NOT NULL,           -- http | socks5
    host TEXT NOT NULL,
    port INTEGER NOT NULL,
    username TEXT,                    -- password disimpan di OS keychain, bukan di sini
    created_at INTEGER NOT NULL
);

CREATE TABLE groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT
);

CREATE TABLE profile_groups (
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    PRIMARY KEY (profile_id, group_id)
);

CREATE TABLE launch_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id TEXT REFERENCES profiles(id) ON DELETE SET NULL,
    launched_at INTEGER NOT NULL,
    closed_at INTEGER,
    pid INTEGER
);
```

Password proxy sengaja tidak disimpan sebagai kolom di tabel `proxies` — disimpan terenkripsi di OS keychain (lihat bagian 7.6) dan diambil saat runtime memakai `proxy_id` sebagai key.

## 7. Modul Backend (Rust)

### 7.1 Profile Manager
Bertanggung jawab atas CRUD profile: validasi nama unik, generate `user_data_dir` baru (misalnya `~/.local/share/multi-browser-manager/profiles/<uuid>/`), dan operasi duplicate (copy metadata dengan folder baru yang kosong, atau opsional clone folder existing kalau user mau "warm clone").

### 7.2 Browser Detector
Trait `BrowserDetector` dengan implementasi berbeda per OS:
- **Linux**: cek PATH via crate `which` untuk `google-chrome`, `google-chrome-stable`, `chromium`, `brave-browser`, `microsoft-edge`
- **Windows**: cek `Program Files`/`Program Files (x86)` dan registry key standar tiap browser
- **macOS**: cek `/Applications/<Browser>.app/Contents/MacOS/<binary>`

Return `Vec<BrowserInfo { name, executable_path, version }>` yang dipakai dashboard untuk populate pilihan browser saat create profile.

### 7.3 Process Orchestrator
- `launch_profile(id)`: build argument list `--user-data-dir=<path>`, `--no-first-run`, `--no-default-browser-check`, plus `--proxy-server=<...>` kalau profile punya proxy tanpa auth, atau `--load-extension=<generated_ext_path>` kalau proxy butuh auth (lihat 7.4). Spawn via `std::process::Command`, simpan `Child` handle di state `Arc<Mutex<HashMap<ProfileId, Child>>>`.
- Task background per child (`tokio::task::spawn_blocking` + `child.wait()`) untuk mendeteksi kapan user menutup browser secara manual, lalu emit event `profile-stopped` ke frontend supaya status di dashboard ikut update tanpa perlu polling.
- `stop_profile(id)`: ambil handle dari state, `child.kill()` (otomatis translate ke SIGKILL di Linux/macOS dan `TerminateProcess` di Windows lewat Rust std).

### 7.4 Proxy Manager
- Proxy tanpa autentikasi: langsung pakai flag `--proxy-server=<protocol>://<host>:<port>`.
- Proxy dengan autentikasi: flag command-line Chromium tidak mendukung format `user:pass@host`. Solusinya generate unpacked extension kecil per profile saat launch (manifest + background script) yang memakai `chrome.proxy.settings.set` untuk set proxy dan listener `chrome.webRequest.onAuthRequired` untuk auto-isi credential saat browser diminta autentikasi oleh proxy server. Extension di-load lewat flag `--load-extension=<path_temp>` dan file-nya digenerate ulang tiap launch supaya credential selalu fresh.
- `test_proxy(id)`: kirim request test (misal ke endpoint IP-checker) lewat proxy tersebut, pakai crate `reqwest` dengan proxy config, return status + response time.

### 7.5 Backup / Export–Import
- Export: serialize seluruh profile + metadata proxy (tanpa password) ke satu file JSON, dengan opsi tambahan enkripsi file pakai passphrase kalau user mau backup yang portable dan aman.
- Import: validasi schema JSON, deteksi konflik nama/path dengan data yang sudah ada, tawarkan opsi merge (skip yang konflik) atau overwrite.

### 7.6 Security Layer
- Password proxy disimpan lewat crate `keyring` (Secret Service di Linux, Keychain di macOS, Credential Manager di Windows) — bukan plaintext di SQLite.
- Tidak ada credential yang ditulis ke log/stdout.
- Folder `user-data-dir` per profile diberi permission ketat (misalnya `700` di Linux) saat dibuat.

## 8. Tauri Commands (IPC Layer)

```rust
// Profile
get_profiles() -> Vec<Profile>
create_profile(input: CreateProfileInput) -> Profile
update_profile(id: String, input: UpdateProfileInput) -> Profile
delete_profile(id: String) -> ()
duplicate_profile(id: String) -> Profile

// Process
launch_profile(id: String) -> LaunchResult
stop_profile(id: String) -> ()
bulk_launch(ids: Vec<String>) -> Vec<LaunchResult>
bulk_stop(ids: Vec<String>) -> ()
get_running_profiles() -> Vec<String>

// Browser
detect_browsers() -> Vec<BrowserInfo>

// Proxy
create_proxy(input: CreateProxyInput) -> Proxy
update_proxy(id: String, input: UpdateProxyInput) -> Proxy
delete_proxy(id: String) -> ()
test_proxy(id: String) -> ProxyTestResult

// Backup
export_profiles(path: String) -> ()
import_profiles(path: String) -> ImportResult
```

Event yang di-emit dari backend ke frontend: `profile-stopped(profile_id)`, `profile-launch-failed(profile_id, reason)`.

## 9. Frontend (Dashboard UI)

- **Dashboard**: grid/list card profile — nama, icon browser, dot status (hijau=running, abu=stopped), tag, tombol launch/stop cepat.
- **ProfileForm**: create/edit profile — nama, pilih browser (dari hasil `detect_browsers`), pilih proxy (dropdown dari daftar proxy tersimpan), tag/group.
- **ProxyManager**: CRUD proxy terpisah dari profile supaya satu proxy bisa dipakai ulang di banyak profile, plus tombol "test connection".
- **State management**: cukup React Context + custom hooks (`useProfiles`, `useProxies`) untuk ukuran project ini — tidak perlu Redux/Zustand kecuali kompleksitas bertambah signifikan.

## 10. Alur Kerja Utama

**Launch profile:**
1. User klik "Launch" di ProfileCard → frontend invoke `launch_profile(id)`
2. Backend ambil data profile + proxy (kalau ada) dari DB
3. Kalau proxy butuh auth → generate extension temp dulu
4. Build argument list → spawn process → simpan handle di state
5. Update status di DB jadi `running` → return `LaunchResult` ke frontend
6. Background task menunggu `child.wait()` untuk deteksi browser ditutup manual

**Stop profile:**
1. User klik "Stop" → invoke `stop_profile(id)`
2. Backend ambil handle dari state → `child.kill()`
3. Update status DB jadi `stopped`, hapus entry dari state

**Import konfigurasi:**
1. User pilih file JSON lewat file dialog (`tauri-plugin-dialog`)
2. Invoke `import_profiles(path)` → backend parse & validasi
3. Kalau ada konflik → frontend tampilkan dialog pilihan merge/overwrite
4. Backend eksekusi sesuai pilihan → refresh list profile di frontend

## 11. Dependencies

**Cargo.toml (backend):**
```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-single-instance = "2"
tauri-plugin-window-state = "2"
tauri-plugin-dialog = "2"
tauri-plugin-updater = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
tokio = { version = "1", features = ["rt-multi-thread", "process", "macros"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
which = "6"
directories = "5"
keyring = "3"
reqwest = { version = "0.12", features = ["socks"] }
thiserror = "1"
anyhow = "1"
```

**package.json (frontend, key deps):** `react`, `react-dom`, `@tauri-apps/api`, `tailwindcss`, `vite`, `typescript`, `lucide-react`.

## 12. Roadmap Pengembangan

| Fase | Fokus |
|---|---|
| 1 — MVP | CRUD profile, launch/stop single profile, dashboard dasar, deteksi browser |
| 2 | Proxy tanpa auth, tag/grouping, search & filter |
| 3 | Bulk launch/stop, export/import JSON |
| 4 | Proxy dengan autentikasi (extension injection), encrypted credential storage |
| 5 | Polish: dark/light theme, keyboard shortcut, auto-updater, launch history |

## 13. Testing Strategy

- **Unit test Rust** (`cargo test`): validasi Profile Manager (nama unik, path handling), Proxy config builder, parsing hasil `detect_browsers` dengan mock filesystem.
- **Integration test**: Tauri command lewat `tauri::test::mock_builder` untuk memastikan IPC layer bekerja sesuai kontrak.
- **Manual test matrix**: kombinasi 3 OS × 4 target browser × proxy on/off, minimal sebelum tiap rilis.

## 14. Build & Distribusi Cross-Platform

`cargo tauri build` menghasilkan installer native per OS:
- **Linux**: `.deb`, `.AppImage`, `.rpm`
- **Windows**: `.msi` (NSIS)
- **macOS**: `.dmg`, `.app`

CI/CD disarankan pakai GitHub Actions matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`) dengan action `tauri-apps/tauri-action` untuk build otomatis tiap tag release, dan `tauri-plugin-updater` untuk auto-update lewat GitHub Releases sebagai update server.

## 15. Pertimbangan Keamanan

- Semua credential proxy lewat OS keychain, tidak pernah plaintext di disk.
- Folder `user-data-dir` per profile permission ketat, terpisah sepenuhnya antar profile.
- Fitur "wipe profile" harus benar-benar menghapus folder di filesystem, bukan cuma menghapus baris di database.
- Export backup: opsional enkripsi file dengan passphrase kalau berisi data sensitif.
