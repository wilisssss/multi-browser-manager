# Multi Browser Manager (MBM)

Desktop app ringan untuk mengelola banyak instance browser Chromium (Chrome, Chromium, Brave, Edge) dari satu dashboard terpusat. Setiap profile punya folder `user-data-dir` sendiri sehingga cookie, session, storage, cache, dan extension **terisolasi penuh** antar profile.

Dibangun dengan **Tauri v2 (Rust)** + **React + Vite + TailwindCSS**.

## Fitur

- **CRUD profile** dengan isolasi data penuh (per-profile `user-data-dir`, permission `700` di Linux)
- **Launch / stop browser** per profile, dengan deteksi otomatis saat browser ditutup manual (event `profile-stopped`)
- **Bulk launch / stop** — tombol "Launch all" / "Stop all" di toolbar
- **System tray** — tutup window hanya menyembunyikan dashboard (browser tetap di-watch); menu tray: Show Dashboard, toggle launch/stop per profile (● running / ○ stopped), **Stop all browsers**, Quit. Ikon tray muncul di status bar apa pun yang mendukung SNI (QuickShell, Waybar, dst.). Butuh `libayatana-appindicator` di Linux; tanpa itu app tetap jalan (tray dilewati). Rebuild menu di-debounce lewat satu worker thread.
- **Tags / groups** — beri label berwarna pada profile, filter by tag di toolbar
- **Pin profile** — profile yang di-pin selalu di atas (urutan: pinned → nama)
- **Deteksi browser** terinstall (Linux / Windows / macOS), termasuk versi
- **Proxy per profile** — HTTP & SOCKS5:
  - Tanpa auth: langsung via flag `--proxy-server`
  - Dengan auth (user/password): otomatis generate unpacked extension sementara yang set proxy + auto-isi credential via `chrome.webRequest.onAuthRequired`
- **Test proxy connection** — cek exit IP + latensi (SOCKS5 auth dikirim via URL credential)
- **Password proxy di OS keychain** (Secret Service / Keychain / Credential Manager) — tidak pernah plaintext di SQLite
- **Credentials per profile** — manajer akun sosial media untuk farming: X (Twitter), Facebook, Discord, Google/Gmail, Instagram, TikTok, Reddit, Telegram, EVM/Bitcoin wallet, + custom
  - Tombol 🔑 di setiap kartu profile → list akun → detail per akun
  - Password, seed phrase & EVM address tersimpan **plaintext di database lokal** (desain sadar untuk penggunaan pribadi: data terproteksi permission folder `0700`, bukan OS keychain) dan **ikut di export/import** untuk migrasi antar device. Jangan export file backup ke tempat yang tidak kamu percaya — siapa pun yang punya file itu punya semua akunmu.
  - Platform picker dengan icon brand; field mengikuti template platform; copy password/seed phrase satu klik dari list
  - Duplicate profile ikut menyalin kredensial; delete profile membersihkan semuanya
- **Export / import** konfigurasi profile + proxy + tags + credentials (JSON), **opsi terenkripsi passphrase** (Argon2id + AES-256-GCM): file terenkripsi tidak bisa dibaca tanpa passphrase; import file terenkripsi otomatis meminta passphrase. Password proxy tidak ikut; **secret kredensial ikut** (lihat peringatan di Credentials) — simpan file backup hanya di tempat terpercaya.
- **Extra launch args per profile** — power-user bisa menambah argumen CLI browser (`--disable-gpu`, `--start-maximized`, URL awal, dst.); flag yang dikelola MBM (`--user-data-dir`, `--proxy-server`, `--load-extension`, `--class`) ditolak saat simpan.
- **Resource usage per profile** — RAM/CPU seluruh process tree browser yang sedang jalan (Linux, via `/proc`), panel Resources di toolbar + badge live di kartu.
- **Statistik usage** — total jam sesi per profile dari launch history; sorting "Most used" + badge di kartu.
- **Restart on crash & stop timeout per profile** — profile farming bisa di-set auto-restart (maks 3x per sesi launch) saat browser exit tidak wajar, plus grace period stop per profile (default 3 detik).
- **Delete ke trash (undo)** — delete memindahkan folder data ke trash + snapshot profile/kredensial; banner Undo 30 detik, snapshot dibersihkan otomatis setelah 30 hari; "Empty trash" ada di Settings.
- **Orphan recovery** — browser yang tertinggal dari sesi sebelumnya terdeteksi saat startup (status `running` kembali benar) dan tetap bisa di-stop. Deteksi ini mengandalkan symlink `SingletonLock` + `/proc` sehingga **hanya berfungsi di Linux**; di Windows/macOS profile yang tertinggal tetap bisa di-launch ulang dengan aman (data tidak korup), hanya statusnya tidak otomatis dikoreksi.
- **Notifikasi desktop untuk CLI** — kegagalan `--launch`/`--stop` dari keybind WM muncul sebagai notifikasi, bukan hanya stderr
- **Dark / light theme** dengan toggle (`T`) + persist di localStorage; dropdown custom (bukan `<select>` native) agar popup selaras dengan dark mode
- **Keyboard shortcuts** in-app (tekan `?` di dashboard untuk daftar)
- **Command palette (`Ctrl+K`)** — cari profile/aksi (launch, stop, edit, export, theme, dst.) dengan navigasi keyboard
- **Launch history** (tombol History / `H`) — durasi sesi per profile
- **Auto-updater** — cek update via tombol di header (tauri-plugin-updater)
- **Settings** — retensi & batas launch history, backup otomatis on/off + jumlah snapshot, default browser type untuk profile baru
- **Auto-snapshot backup** — sekali sehari (jika berubah) snapshot profile+proxy+credentials ke `~/.local/share/multibrowsermanager/backups/`, auto-prune. Folder data sudah `0700`, tapi ingat snapshot berisi secret kredensial (lihat peringatan Credentials).
- **Window rules & workspaces niri** — setiap browser diluncurkan dengan app-id unik (`--class`/`--wayland-app-id` = `mbm-<nama>-<id>`); modal khusus menampilkan app-id per profile, mapping tag → workspace, dan generator snippet `window-rule { match app-id=... open-on-workspace ... }`
- **CLI + WM keybinds** — launch/stop profile langsung dari keybind compositor (niri, dst.), fuzzel menu dengan indikator status ●/○

## CLI & Keybinds niri

Binary mendukung argument:

```bash
multi-browser-manager                # buka / focus dashboard (single-instance)
multi-browser-manager --list         # print daftar profile sebagai JSON
multi-browser-manager --launch work  # launch profile bernama "work"
multi-browser-manager --stop work    # stop profile bernama "work"
```

Contoh keybind niri (salin ke block `binds { ... }` di `~/.config/niri/config.kdl` — lihat juga `docs/niri-keybinds.kdl`):

```kdl
Mod+B       { spawn "multi-browser-manager"; }
Mod+Shift+B { spawn "multi-browser-manager" "--launch" "work"; }
Mod+Shift+X { spawn "multi-browser-manager" "--stop" "work"; }
```

`--launch` dari proses kedua diteruskan ke instance yang sudah berjalan via single-instance plugin, dan dashboard otomatis refresh (event `profiles-changed`).

## Struktur

```
src-tauri/src/
├── main.rs / lib.rs        # entry point, state, plugin, invoke handler
├── commands/               # Tauri IPC commands
│   ├── profile.rs          # CRUD profile
│   ├── process.rs          # launch/stop + process watcher
│   ├── proxy.rs            # CRUD proxy + test
│   └── backup.rs           # export/import JSON
├── browser/
│   ├── detector.rs         # deteksi browser per OS
│   └── launcher.rs         # build args + spawn/kill process
├── db/                     # rusqlite + migrations
├── models/                 # Profile, Proxy structs
├── proxy_manager.rs        # keychain, extension generator, proxy test
├── startup.rs              # rekonsiliasi status & prune history (unit-tested)
└── error.rs                # error type serializable ke IPC

src/                        # frontend React
├── components/             # Dashboard (orchestrator), DashboardHeader, Toolbar,
│                           # Banners, ProfileList/Card, ProfileForm, ProxyManager, dst.
├── hooks/                  # useProfiles, useProxies, useProfileFilters (search+filter+sort)
└── lib/tauri-api.ts        # typed invoke wrappers
```

## Menjalankan (development)

```bash
npm install
npm run tauri dev      # menjalankan vite dev server + app
```

## Build binary standalone / release

```bash
npm run tauri build    # release build + installer (.deb / .AppImage / .rpm)
```

Binary debug yang bisa jalan sendiri (UI ter-embed dari `dist/`, tanpa dev server):

```bash
npm run app   # = npm run build + cargo build --features custom-protocol
```

> **Penting:** selalu build standalone binary via `npm run app`. Cargo tidak
> mengingat feature antar-build — kalau ada build lain berjalan tanpa feature
> `custom-protocol` (mis. `cargo build` polos atau `cargo test`), binary berikutnya
> bisa ter-build dalam mode dev: app akan mencoba memuat UI dari
> `http://localhost:5183` (devUrl) dan menampilkan "Could not connect to
> localhost: Connection refused" kalau `npm run dev` tidak sedang berjalan.
> Solusinya: jalankan lagi `npm run app` lalu restart `mbm`.

## Testing

```bash
cd src-tauri && cargo test   # unit test: launcher args, crypto backup, import/trash engine, resources, settings, proses
npx tsc --noEmit             # type-check frontend
npm test                     # unit test frontend (format helper)
npm run build                # type-check + build frontend
```

## Catatan penting

- **Chrome branded 137+** membatasi flag `--load-extension`, jadi proxy **dengan autentikasi** mungkin tidak jalan di Chrome stable terbaru. Gunakan **Chromium atau Brave** untuk proxy auth, atau Chrome dev/canary. Proxy **tanpa** auth aman di semua browser (pakai `--proxy-server` biasa).
- Password proxy hanya di memori saat generate extension dan dihapus dari disk (`~/.cache/multibrowsermanager/proxy-ext/`) begitu profile berhenti; browser yang masih hidup saat MBM keluar tidak disapu ekstensinya (proxy auth-nya tetap berfungsi).
- Saat startup, status tiap profile direkonsiliasi dengan kenyataan: browser orphan dari sesi sebelumnya (SingletonLock masih hidup) ditandai `running` kembali dan tetap bisa di-stop; lock dari browser yang sudah mati dibersihkan otomatis.
- Tray di Linux butuh `libayatana-appindicator` (`sudo pacman -S libayatana-appindicator` di Arch). Tanpa paket itu app tetap jalan, hanya tanpa ikon tray.
- Test proxy SOCKS5 dengan auth mengirim credential lewat URL (`socks5://user:pass@host:port`) — karakter khusus di password sebaiknya di-percent-encode.
- Panel **Resource usage** hanya tersedia di Linux (dibaca dari `/proc`); di OS lain daftarnya kosong dan badge RAM/CPU disembunyikan.
- **CSP** produksi dikunci (`default-src 'self'` di `tauri.conf.json`) — webview tidak boleh memuat resource eksternal; ini menutup vektor XSS→ekfiltrasi secret kredensial dari modal credentials.

## Auto-updater (setup release)

Updater sudah ter-wire (tauri-plugin-updater + tombol "Check updates" di header). Untuk rilis:

1. Endpoint update dikonfigurasi di `src-tauri/tauri.conf.json` → `plugins.updater.endpoints` — ganti `YOUR_USER` dengan repo GitHub kamu.
2. Public key sudah ter-set di config; **private key** signing ada di `~/.tauri/mbm-updater.key` (JANGAN di-commit). Saat build release:
   ```bash
   TAURI_SIGNING_PRIVATE_KEY=~/.tauri/mbm-updater.key TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" npm run tauri build
   ```
3. Upload artefak + `latest.json` (dihasilkan updater) ke GitHub Releases.
4. Di Linux, updater hanya mendukung bundle **AppImage** (deb/rpm update via package manager).

### Release via GitHub Actions (otomatis)

`.github/workflows/release.yml` mem-build AppImage + .deb dan membuat GitHub Release
(lengkap dengan `latest.json` untuk auto-updater) setiap kali tag `v*` di-push.

Setup sekali:

1. Buat repo GitHub, lalu:
   ```bash
   git remote add origin git@github.com:USERNAME/multi-browser-manager.git
   git push -u origin main
   ```
2. Ganti `YOUR_USER` di `src-tauri/tauri.conf.json` → `plugins.updater.endpoints`
   dengan username GitHub kamu.
3. Tambahkan **Secrets** di repo (Settings → Secrets and variables → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY` → isi file `~/.tauri/mbm-updater.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` → kosongkan (key dibuat tanpa password)
4. Rilis:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
   CI akan membuat release + aset: `*.AppImage`, `*.AppImage.sig`, `*.deb`,
   `*.deb.sig`, dan `latest.json` (URL updater: `.../releases/latest/download/latest.json`).

## Keyboard shortcuts (in-app)

| Key | Aksi |
|---|---|
| `Ctrl+K` | **Command palette** — cari & jalankan aksi/profile apa saja |
| `/` | Focus search |
| `N` | New profile |
| `P` | Toggle proxy manager |
| `H` | Toggle launch history |
| `T` | Toggle dark/light theme |
| `R` | Refresh profiles |
| `?` | Bantuan shortcut |
| `Esc` | Tutup dialog |
