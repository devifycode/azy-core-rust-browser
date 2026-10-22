# Azy Core

Browser Windows x64 dengan aplikasi native Rust dan mesin Microsoft Edge WebView2 (Chromium). Antarmuka mengikuti pola Chrome: tab di atas, omnibox, toolbar navigasi, dan bilah bookmark. Logo berasal dari `logo-azy-core.png` dan tertanam di executable.

## Menjalankan

Unduh **AzyCore.exe** dari [GitHub Releases](https://github.com/devifycode/azy-core-rust-browser/releases/latest). Executable tersedia sebagai aset release; repository berisi kode sumber dan skrip pengujian. `SHA256SUMS.txt` pada release dapat dipakai untuk memeriksa integritas unduhan.

Klik dua kali **AzyCore.exe** di folder ini. Memerlukan Windows 10/11 x64 dan Microsoft Edge WebView2 Runtime. Runtime sudah terdeteksi pada komputer tempat aplikasi ini dibuat. Tidak memerlukan instalasi Rust untuk menjalankan aplikasi.

Simpan executable di lokasi yang dapat ditulis. Profil, cookies, cache, bookmark, sesi, riwayat, dan unduhan disimpan dalam **AzyCore.Data** di samping executable. Unduhan berada di **AzyCore.Data\Downloads**. Logo dan antarmuka sudah ditanam ke executable; file sumber logo tidak harus ikut disalin.

## Fitur

- Seret tab ke kiri atau kanan untuk mengubah urutannya. Pratinjau tab beserta ikon dan judul mengikuti pointer dengan bayangan, sementara posisi asal diredupkan. Penanda menunjukkan posisi tujuan dan bilah tab bergeser otomatis saat diseret mendekati tepi. Tab dipin tetap dalam kelompok paling kiri; urutan ikut pemulihan sesi.
- Pasangan split ditempatkan berdampingan di bilah tab dan tetap berdekatan setelah drag, pin/unpin, atau tukar posisi. Memindahkan salah satu tab split ikut memindahkan pasangannya. Jika hanya satu tab pasangan dipin, keduanya bertemu di batas antara kelompok pin dan tab biasa tanpa mengubah preferensi pin. Modal pemilih split memakai region native membulat yang mengikuti kontainernya tanpa pita di luar sisi/sudut.
- **Split view:** buka dua tab website, klik tombol split di toolbar lalu pilih tab pasangan. Bisa juga klik kanan tab lain dan pilih **Buka berdampingan dengan tab aktif**. Geser pembatas tengah untuk mengatur lebar (25–75%); klik dua kali untuk kembali seimbang. Klik halaman atau tab untuk memilih panel yang dikendalikan kolom alamat, navigasi dan zoom. Menu split menyediakan **Tukar posisi kiri / kanan** serta **Keluar split view**. Menutup satu panel mempertahankan tab pasangannya pada lebar penuh. Pasangan split berlaku selama aplikasi berjalan; memilih tab lain menampilkan tab itu penuh dan memilih salah satu pasangan mengembalikan split. Website biasa dan privat tidak dapat dipasangkan bersama; Home bukan panel split. Kedua halaman yang terlihat tidak ditidurkan oleh penghemat memori.
- Kolom alamat dan pencarian Home menampilkan hingga enam saran dari riwayat lokal, berdasarkan judul atau alamat. Pilih dengan klik, tombol panah dan Enter; Esc menutup daftar. Dropdown tidak mengubah ukuran website. Tidak ada permintaan saran ke layanan pencarian.
- Perubahan zoom halaman, termasuk Ctrl+scroll, menampilkan persentase zoom pada toolbar selama 1,8 detik. Tombol **Reset** mengembalikan zoom ke 100%.
- Klik kanan tab lalu **Pin tab** untuk memindahkannya ke kelompok paling kiri dengan tampilan ikon saja. **Lepas pin tab** mengembalikan judulnya. Pin ikut pemulihan sesi bila diaktifkan; pin pada tab privat tidak disimpan. Tab pin bisa ditutup melalui menu klik kanan, klik tengah atau Ctrl+W.

- Ikon puzzle membuka popover berisi ekstensi aktif; klik lagi untuk menutup. Menu **Setelan lanjutan ekstensi** berada paling bawah dan membuka pengelola pemasangan, izin, aktif/nonaktif, hapus, serta halaman opsi.
- Tombol pin di kanan setiap ekstensi mengatur apakah ikonnya ditampilkan pada toolbar. Pin/unpin tidak mengubah status aktif; ekstensi tanpa pin tetap bisa dibuka melalui daftar. Pilihan disimpan setelah restart, termasuk untuk ekstensi background. Ikon yang sudah tampil tetap ditampilkan sampai dilepas pinnya. Batas native popover mengikuti sudut container tanpa bayangan/pita di luarnya.
- Chrome Web Store: buka halaman detail ekstensi dan klik **Pasang ke Azy Core** pada toolbar, atau tempel tautan/ID di Setelan lanjutan. Azy Core mengambil paket dari endpoint resmi Google, memverifikasi tanda tangan penerbit dan pengembang serta ID, kemudian menampilkan izin sebelum pengguna menekan **Pasang ekstensi**. Sudah diuji dengan Google Translate dari Web Store.
- Ikon masing-masing ekstensi aktif tampil di toolbar. Klik untuk membuka `action.default_popup` / `browser_action.default_popup` pada website biasa. Popup membaca tab website yang dipilih melalui identitas window native, termasuk saat URL yang sama dibuka di beberapa tab. Tutup dengan klik ikon yang sama, Esc, klik di luar popup, atau pindah tab; tidak ada tombol X tambahan. Ikon/popup instalasi lama dibaca otomatis dari manifest setelah restart.
- Tema gelap berlaku pada antarmuka browser dan Home. Warna dasar WebView website tetap putih dan tidak diubah saat tema aplikasi berganti; CSS dan preferensi tema website tetap mengikuti website/OS, termasuk tema gelap yang memang disediakan situs.
- Ekstensi Chromium berjalan pada tab biasa. UI aplikasi menggunakan direktori profil terpisah dengan ekstensi dimatikan; tab privat memakai profil incognito di lingkungan tersebut. Ekstensi pengguna tidak dijalankan pada UI aplikasi atau tab privat.

- Home dapat dipersonalisasi: tambah pintasan website, edit nama/alamat, dan hapus pintasan melalui ikon pensil. Mendukung hingga 100 pintasan dan alamat HTTP/HTTPS, termasuk portal lokal.
- Pintasan menampilkan favicon website. Ikon dari tab yang pernah dibuka disimpan untuk origin pintasan; ini mendukung ikon dengan lokasi khusus, termasuk Water Solution. Pintasan baru mencoba `/favicon.ico` langsung dari website (Gmail memakai aset favicon resmi Google). Jika ikon belum tersedia, huruf awal nama tetap tampil. Permintaan gambar tidak memakai layanan pencari favicon pihak ketiga, tidak mengirim Referer, dan dimuat secara lazy. Ikon tab privat tidak ditulis ke cache pintasan.
- Tombol **Sesuaikan Home** di kanan bawah membuka palet warna, pemilih warna sendiri, unggah gambar PNG/JPG/WebP, hapus gambar, dan kembali mengikuti tema. Pilihan disimpan setelah restart. Gambar disalin ke profil, sehingga file aslinya tidak perlu tetap berada di lokasi awal.

- Pemblokir iklan/pelacak berbasis domain, aktif secara default, termasuk tab privat. Ikon perisai membuka penghitung per halaman, sakelar global, dan pengecualian per hostname. Pengecualian situs disimpan dan halaman dimuat ulang saat diubah.
- Filter offline: 49.470 domain dari subset EasyList/EasyPrivacy serta tambahan Azy Core, dibagikan antar-tab tanpa extension tambahan. Snapshot dan atribusi tersedia di `assets/filters/`. Daftar belum diperbarui otomatis. Bukan interpreter penuh EasyList; iklan dari domain konten yang sama, iklan video tertentu, dan ruang iklan kosong dapat tetap tampil. Tidak ada penyembunyian elemen CSS generik yang berisiko menyembunyikan konten situs.

- Banyak tab dengan favicon asli website, tutup/buka kembali tab, tautan tab baru, dan jendela baru. Favicon mengikuti perubahan ikon situs; perubahan rute `#...` tidak membatalkan pengambilan ikon. Hasil kosong sementara tidak menghapus ikon yang sudah tampil. Ikon diperiksa lagi setelah navigasi selesai dan PNG besar diperkecil maksimal 64 × 64. Situs tanpa favicon memakai ikon bawaan.
- Alamat URL atau pencarian Google/Bing/DuckDuckGo.
- Kembali, maju, reload, stop, beranda, zoom, layar penuh.
- Bookmark persisten dan bilah bookmark yang tersembunyi secara default; tampilkan melalui Setelan atau Ctrl+Shift+B. Riwayat dengan pencarian.
- Unduhan dengan pelacakan selesai/gagal dan folder lokal.
- Tab privat dengan cookies terpisah; tidak dicatat dalam riwayat/sesi. Bookmark dan file unduhan tetap tersimpan.
- Temukan teks pada halaman, cetak, buka file lokal, developer tools.
- Tema terang/gelap, pilihan mesin pencari, pemulihan tab biasa.
- Tab baru kosong tidak membuat WebView tambahan. Website dari sesi sebelumnya baru dimuat saat tabnya dipilih.
- Saat pertama membuka situs, area transisi polos mengikuti tema tanpa ikon loader atau tulisan. WebView ditampilkan setelah dokumennya terbentuk dan jeda render 120 ms. Batas tunggu 3 detik menjaga halaman lambat/error tetap dapat diakses. Warna dasar jendela dan WebView mengikuti tema untuk mengurangi kilatan hitam/putih; warna dan animasi asli situs tidak diubah. Event navigasi dicocokkan lewat ID agar event halaman kosong lama tidak menyelesaikan halaman baru.
- Hemat memori tab aktif secara default: tab latar belakang yang memenuhi syarat ditidurkan setelah 60 detik dan dilanjutkan saat dipilih. Isi halaman/formulir tetap dipertahankan; tidak dilakukan reload otomatis. Matikan **Setelan → Hemat memori tab** bila membutuhkan aktivitas latar belakang terus-menerus. Tab yang sedang memuat, memutar audio, atau saat ada unduhan aktif dikecualikan; WebView2 dapat menolak tidur untuk aktivitas lain seperti WebRTC.
- Izin situs, TLS, dialog file situs, dan rendering media menggunakan WebView2.

## Pintasan

| Pintasan | Fungsi |
| --- | --- |
| Ctrl+T / Ctrl+W | Tab baru / tutup tab |
| Ctrl+Shift+T | Buka kembali tab tertutup |
| Ctrl+Shift+N | Tab privat baru |
| Ctrl+N | Jendela baru |
| Ctrl+Tab / Ctrl+Shift+Tab | Tab berikutnya / sebelumnya |
| Ctrl+L | Fokus alamat |
| Alt+← / Alt+→ | Kembali / maju |
| Ctrl+R atau F5 | Reload |
| Ctrl+D / Ctrl+H / Ctrl+J | Bookmark / riwayat / unduhan |
| Ctrl+Shift+B | Tampilkan / sembunyikan bilah bookmark |
| Ctrl+F / Ctrl+P / Ctrl+O | Temukan / cetak / buka file |
| Ctrl+plus / Ctrl+minus / Ctrl+0 | Zoom |
| Ctrl+scroll atas / bawah | Perbesar / perkecil halaman website |
| F11 / Esc | Layar penuh / kembali |

## Batas versi 0.1

Ini browser awal yang dapat digunakan, bukan implementasi penuh Google Chrome. Badge toolbar dinamis, action tanpa popup (`action.onClicked`), sinkronisasi akun Google, pengelola kata sandi khusus, impor profil Chrome, registrasi default browser, dan pembaruan otomatis aplikasi/ekstensi belum tersedia. Pemasangan Chrome Web Store memakai tombol Azy Core; tombol bawaan **Add to Chrome** pada halaman toko tidak terintegrasi dengan WebView2. Paket terbatas atau yang memerlukan autentikasi mungkin tidak tersedia. Tidak semua API ekstensi Chrome tersedia. Penyesuaian tab saat ini berlaku pada konteks popup; API window/tab dari background worker tetap mengikuti model window WebView2. Dukungan DRM, codec, serta autentikasi popup tertentu bergantung pada WebView2. Pencarian teks memakai `window.find`, sehingga dukungannya mengikuti halaman. Jendela berbeda berbagi folder profil; pengaturan yang diubah serentak pada beberapa jendela belum disinkronkan secara langsung.

## Memasang ekstensi

1. Siapkan folder ekstensi Chromium yang sudah diekstrak dan berisi `manifest.json` di tingkat teratas. Pilih ekstensi dari sumber yang Anda percaya sesuai izin manifest.
2. Klik ikon puzzle **Ekstensi** → **Setelan lanjutan ekstensi** → **Pilih folder**, atau masukkan lokasi folder dan klik **Pasang**.
3. Gunakan sakelar **Aktif**, **Hapus**, atau **Opsi ekstensi** bila manifest menyediakan halaman opsi.
4. Muat ulang tab untuk menerapkan pemasangan atau perubahan status. Menonaktifkan ekstensi tidak membatalkan perubahan DOM yang sudah dilakukan pada dokumen lama.
5. Kembali ke website, kemudian klik ikon ekstensi di toolbar. Tombol dinonaktifkan pada Home dan tab privat. Popup menjalankan berkas ekstensi asli pada profil website, tanpa akses perintah internal browser. Unduhan ekstensi memakai folder `AzyCore.Data/Downloads`.

Folder sumber tidak disalin oleh WebView2: jangan pindahkan atau ubah isinya setelah pemasangan. Perubahan file sumber dapat membuat runtime melepas ekstensi; pasang kembali dari folder untuk memuat versi lain. File ZIP/CRX perlu disiapkan sebagai folder unpacked lebih dahulu. Manifest V3 dengan content script, background service worker, penyimpanan, halaman opsi dan popup telah diuji; kompatibilitas ekstensi lain mengikuti runtime. Halaman opsi dibuka pada tab ekstensi; popup dibuka dari ikon toolbar tanpa mengganti tab website. Status aktif dan pemasangan bertahan setelah aplikasi dimulai ulang.

Paket Web Store dikelola otomatis di `AzyCore.Data/Extensions`. Verifikasi CRX3 memakai Windows Cryptography, SHA-256, bukti pengembang yang cocok dengan ID, dan kunci penerbit produksi Google yang dipakai Chromium. Ekstraksi menolak traversal path, symlink, duplikat nama, CRC/ukuran salah, serta paket yang melebihi batas. Manifest diberi public key terverifikasi untuk mempertahankan ID asli saat dimuat sebagai unpacked. Unduhan/dekompresi berjalan di worker, bukan thread UI. **Batal** menghapus paket yang belum dipasang; paket terpasang tidak dimodifikasi. Hapus lalu pasang ulang untuk mengambil versi lebih baru. Paket dibatasi 100 MiB, hasil ekstraksi 500 MiB, dan tiap berkas 64 MiB; ZIP64 dan format CRX lama tidak didukung. Dasar format: [Chromium CRX3](https://chromium.googlesource.com/chromium/src/+/main/components/crx_file/crx3.proto) dan [verifikasi Chromium](https://chromium.googlesource.com/chromium/src/+/main/components/crx_file/crx_verifier.cc).

Profil website dan ekstensi tetap di `AzyCore.Data/WebView2`, sehingga cookies website lama tetap memakai lokasi sebelumnya. UI dan incognito memakai lingkungan terpisah di `AzyCore.Data/WebView2.UI` dengan ekstensi dimatikan. Metadata folder/versi/izin berada di `browser.json`. Tutup semua jendela versi lama sebelum pertama kali menjalankan versi dengan ekstensi, agar opsi lingkungan WebView2 tidak bercampur pada profil yang sama.

Dasar dukungan dan batasan API: [Microsoft WebView2 — pengelolaan ekstensi](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2profile7). Pengelola membutuhkan runtime yang menyediakan API ekstensi (mulai WebView2 120). Runtime yang tidak mendukungnya akan menampilkan pesan pada halaman Ekstensi.

## Personalisasi Home

1. Di Home/tab baru, klik **Tambah pintasan**, isi nama dan alamat website, lalu **Simpan**. Alamat seperti `example.com` otomatis memakai HTTPS.
2. Arahkan mouse ke pintasan dan klik ikon pensil untuk mengedit. Tombol **Hapus** tersedia di dialog yang sama.
3. Klik **Sesuaikan Home** di kanan bawah untuk mengganti background Home/tab baru dengan warna atau gambar dari perangkat. Pengaturan ini juga dapat dibuka melalui Setelan.
4. Gunakan **Hapus gambar** untuk kembali ke warna pilihan, atau **Ikuti tema browser** untuk mengembalikan background bawaan.

Gambar input maksimal 25 MB dan disesuaikan hingga 1920 × 1080 tanpa memperbesar gambar kecil. Salinan PNG disimpan sebagai `AzyCore.Data/home-background.png`; daftar pintasan dan warna berada di `AzyCore.Data/browser.json`. Personalisasi tersimpan untuk profil yang sama, termasuk ketika diubah dari tab privat. Gambar tidak dimuat dari layanan eksternal.

## Build lokal

Untuk clone baru, siapkan Rust stable target `x86_64-pc-windows-msvc`, Visual Studio Build Tools dengan workload C++ dan Windows SDK. Jalankan dari Developer PowerShell:

```powershell
New-Item -ItemType Directory -Force .cache/tmp | Out-Null
$env:CARGO_HOME = Join-Path $PWD '.cache/cargo'
$env:CARGO_TARGET_DIR = Join-Path $PWD 'target'
$env:TEMP = Join-Path $PWD '.cache/tmp'
$env:TMP = $env:TEMP
$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --release
Copy-Item target/release/AzyCore.exe AzyCore.exe
```

Build pertama memerlukan jaringan untuk mengambil dependencies. Skrip `scripts/build.ps1` di bawah memakai toolchain/SDK lokal khusus lingkungan pengembangan awal, yang dikecualikan dari Git. Cache, profil browser, hasil build, installer dan executable cadangan juga dikecualikan melalui `.gitignore`.

Seluruh toolchain Rust dan paket unduhan build berada di **.cache**. File hasil kompilasi berada di **target**; TEMP/TMP build juga diarahkan ke **.cache\tmp**. Skrip memakai tool linker Visual Studio yang sudah tersedia dan Windows SDK/CRT resmi yang diekstrak secara lokal; tidak mengubah PATH sistem.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build.ps1 -Release
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build.ps1 -Action test -Release
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-browser.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-presentation.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-home.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-favicon.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-extensions.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-browser-update.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-extensions-update.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-extension-pins.ps1
```

Build release menyalin `target\release\AzyCore.exe` ke `AzyCore.exe`. Cargo.lock menyimpan versi dependensi. C runtime ditautkan statis. Folder `.cache` dan `target` tidak dibutuhkan pengguna aplikasi, tetapi diperlukan untuk build ulang offline pada komputer ini.

Skrip integrasi `scripts/smoke.cjs` menguji executable melalui endpoint DevTools sementara pada port dinamis lokal dan server pengujian localhost port 3137. Port ditemukan dari `DevToolsActivePort` masing-masing lingkungan UI/website sehingga pengujian juga menjangkau profil yang terpisah. Hasil dan pratinjau disimpan di `.cache`. Endpoint debugging tidak diaktifkan saat aplikasi dijalankan biasa. `AZY_CORE_DATA_DIR` mengisolasi profil pengujian.

Sumber teknologi: [Wry](https://github.com/tauri-apps/wry), [Tao](https://github.com/tauri-apps/tao), [Microsoft WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/).

## Verifikasi build ini

Dua puluh kelompok uji `scripts/test-tabs-split.ps1` memverifikasi drag pointer nyata, pratinjau tab yang mengikuti pointer dan hilang setelah dilepas, urutan serta batas kelompok pin, pasangan split yang tetap berdekatan termasuk saat drag/pin campuran, region modal tanpa area di luar sisi/sudut, state halaman yang tetap, dua viewport website, fokus Windows pada kedua panel, region chrome yang menyisakan area website interaktif, drag divider, tukar posisi, kontrol panel aktif, perpindahan ke tab lain/setelan, tutup/keluar split, pemisahan mode privat, menu klik kanan, urutan setelah restart, dan pemuatan tab sesi yang masih tertunda. Probe fokus dibatasi ke proses browser pengujian dan fixture lokal memakai profil sementara di `.cache`.

Delapan kelompok uji `scripts/test-extension-pins.ps1` memverifikasi region window membulat tanpa area di luar sisi/sudut modal, pin/unpin toolbar, pin ekstensi background yang tidak menyediakan popup, akses popup tanpa pin, content script yang tetap berjalan, pemulihan preferensi setelah restart, dan penghapusan preferensi saat ekstensi dihapus. Pemeriksaan region memakai koordinat serta DPI window asli, dengan fixture ekstensi pada profil sementara.

Tiga belas kelompok uji `scripts/test-extensions-update.ps1` memverifikasi menu daftar/toggle tanpa X, tautan setelan lanjutan, penolakan URL palsu, unduhan/verifikasi paket Google Translate resmi, tinjauan izin sebelum pemasangan, pembatalan, tombol pada halaman detail Web Store, ID asli dan status setelah restart, aktif/nonaktif, serta isolasi tema antarmuka dari website. Pengujian menggunakan profil sementara; membutuhkan jaringan untuk endpoint resmi Google. Unit test juga memverifikasi paket bertanda tangan asli, penolakan ID yang salah dan paket yang byte-nya diubah.

Sebelas kelompok uji `scripts/test-browser-update.ps1` memverifikasi saran riwayat di Home/kolom alamat, judul yang di-escape, deduplikasi URL, pemilihan dengan keyboard dan klik, Esc, viewport website yang tetap, region dropdown Windows, indikator/reset zoom dan timer, pin/unpin dari menu klik kanan, buka kembali tab pin, pemulihan setelah restart, serta pengecualian data privat. Indikator zoom diuji melalui perubahan zoom native yang dipicu perintah browser; input Ctrl+scroll fisik belum diuji otomatis.

Lima uji transisi halaman pada `scripts/test-presentation.ps1` lulus: permukaan pemuatan sebelum respons tiba, halaman gelap ditampilkan tanpa menunggu gambar lambat, pemakaian ulang WebView dari tab baru, tema gelap aplikasi, dan fallback respons lambat. Pengujian mengamati batas viewport dan status presentasi dengan fixture lokal; bukan pengukuran frame compositor GPU pada seluruh website. Rekaman laporan pengguna telah diperiksa; warna serta skeleton pemuatan yang dibuat website tetap mengikuti website tersebut.

Build release berhasil; 11 unit test dan 9 kelompok uji integrasi browser lulus. Uji integrasi memverifikasi startup/logo, rendering halaman, penolakan perintah aplikasi dari halaman web, navigasi kembali/maju, bookmark/riwayat, zoom, tab baru/tutup/pulihkan, cookies privat terpisah, unduhan file aktual, halaman internal, dan penyimpanan data. Pemeriksaan DLL menunjukkan executable hanya bergantung pada library sistem Windows; mesin WebView2 tetap harus terpasang. Dialog cetak/file, pintasan keyboard fisik, dan kompatibilitas seluruh situs internet belum diuji menyeluruh.

Uji ekstensi memakai ekstensi Manifest V3 buatan sendiri pada profil sementara: pemasangan lewat formulir, content script di website, komunikasi dengan service worker, isolasi dari UI aplikasi, halaman opsi, sakelar nonaktif/aktif, penghapusan cookies profil website, isolasi tab privat, status nonaktif setelah restart, dan penghapusan dari profil. Folder picker sistem disediakan melalui Windows FileOpenDialog; otomatisasi memakai formulir lokasi folder. Komponen internal runtime PDF/clipboard tidak ditampilkan sebagai ekstensi pengguna.

Tujuh kelompok uji tambahan `scripts/test-grabber.ps1 -ExtensionFolder '<folder 3D Asset Grabber>'` memakai ekstensi GLB asli tanpa mengubah folder sumber: ikon toolbar, popup pada website aktif dengan URL duplikat, query tab Promise/callback, deteksi aset tanpa akses perintah UI, unduhan GLB dengan verifikasi byte, penutupan popup, dan pemulihan ikon setelah restart dari metadata format lama. Uji memakai server lokal dan profil terpisah di `.cache`; kompatibilitas konversi seluruh format 3D dan seluruh website belum diuji.

Empat kelompok uji favicon `scripts/test-favicon.ps1` memverifikasi beberapa deklarasi ikon relatif ICO/PNG/SVG, pergantian hash URL, pergantian ke SVG besar, tab baru dengan cache ikon, serta reload. Unit test memeriksa stream yang posisi bacanya sudah bergeser dan normalisasi ikon besar. Pemeriksaan situs asli tanpa login memverifikasi ikon tampil pada halaman login Water Solution; dashboard yang memerlukan akun tidak diakses.

Sembilan kelompok uji `scripts/test-home.ps1` memverifikasi dialog tambah/edit/hapus pintasan, tautan membuka website, favicon langsung dari website, cache ikon dengan lokasi khusus yang tetap tampil setelah restart, penolakan skema URL yang tidak didukung, kontras warna pada kedua tema, unggah gambar tanpa memperbesar gambar kecil, penyimpanan setelah restart, serta hapus/reset background. Unit test juga memverifikasi migrasi profil lama dan daftar pintasan kosong yang sengaja disimpan. Pemilihan file pengujian memakai input file WebView2 melalui DevTools; klik dialog file sistem secara fisik belum diuji.

Tiga kelompok uji pemblokir pada `scripts/test-adblock.ps1` memverifikasi iklan/pelacak/iframe/worker terhenti sebelum mencapai server, konten normal tetap dimuat, pengecualian situs tersimpan dan mengizinkan request nyata, serta sakelar global dan tab privat. Fixture mengarahkan domain filter ke server lokal; pengujian tidak bergantung pada jaringan iklan eksternal. Kebijakan sertifikat/sandbox browser tidak diturunkan. `assets/filters/NOTICE.md` memuat atribusi dan lisensi CC BY-SA 3.0 atau versi lebih baru untuk data filter yang diadaptasi dari The EasyList authors.

Uji tambahan `scripts/test-session.ps1` memverifikasi pemulihan sesi tanpa pemuatan website latar belakang, pemuatan ketika tab dipilih, pengaktifan tab tertunda setelah tab aktif ditutup, dan bookmark tetap tersimpan. Benchmark `scripts/benchmark.ps1` juga memverifikasi tiga tab tidur dan isi formulir tetap ada setelah dilanjutkan tanpa permintaan halaman ulang. Lihat [PERFORMANCE.md](PERFORMANCE.md) untuk angka dan batas pengukurannya.
