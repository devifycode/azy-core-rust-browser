# Optimasi Azy Core — 21 Oktober 2026

Engine tetap Microsoft Edge WebView2/Chromium. Implementasi aplikasi dioptimalkan untuk mengurangi WebView yang tidak perlu dan pekerjaan berulang.

Catatan versi ekstensi: angka di bawah diukur sebelum profil UI dan website dipisahkan. Versi dengan ekstensi memisahkan lingkungan UI/privat yang menonaktifkan ekstensi dari profil website yang mendukung ekstensi. Ini dapat menambah proses dan RAM. Host pengelola dibuat saat dibutuhkan, bukan untuk setiap tab kosong. Angka historis berikut belum merupakan benchmark versi dengan ekstensi.

## Perubahan

- Tab baru kosong memakai halaman awal pada UI yang sudah ada; tidak ada renderer website tambahan.
- Tab sesi lama dipulihkan sebagai metadata. Hanya tab yang dipilih memuat website.
- Tab latar belakang yang memenuhi syarat tidur setelah 60 detik menggunakan API native `TrySuspend`; saat dipilih dilanjutkan dengan `Resume`, mempertahankan halaman, riwayat, dan isi formulir. Proses ini bukan membuang tab atau memuat ulang website.
- Tab yang sedang memuat atau memutar audio dikecualikan. Tidur ditunda selama ada unduhan aktif. WebView2 dapat menolak tidur untuk aktivitas tertentu. Pengguna dapat menonaktifkannya di Setelan.
- Ukuran permukaan UI diperkecil menjadi area toolbar ketika website ditampilkan.
- Data UI hanya dikirim jika berubah. Riwayat dan unduhan tidak dikirim ulang untuk setiap favicon, zoom, atau perubahan tab.
- DOM toolbar/tab tidak dibuat ulang bila markup tidak berubah. Halaman setelan/riwayat hanya dirender ulang saat datanya berubah.
- Profil tidak ditulis ulang ke disk bila data tersimpan belum berubah.

## Pengukuran lokal

Skenario identik, dijalankan berurutan dengan profil baru pada komputer yang sama. Semua proses turunan executable uji dihitung; proses browser milik pengguna tidak dihitung. Angka satu kali pengukuran, bukan rata-rata benchmark lintas komputer.

Angka ini dicatat pada build optimasi sebelum fitur pemblokir iklan ditambahkan. Build terbaru juga memuat snapshot aturan domain; tabel ini bukan pengukuran ulang build pemblokir iklan.

| Skenario | Sebelum: working set total | Sesudah: working set total | Proses sebelum → sesudah |
| --- | ---: | ---: | ---: |
| Awal, satu tab kosong | 404,9 MiB | 359,9 MiB | 8 → 7 |
| 13 tab kosong | 1.004,3 MiB | 363,9 MiB | 20 → 7 |
| 13 tab kosong + 4 halaman fixture | 1.295,6 MiB | 655,2 MiB | 24 → 11 |
| Skenario di atas setelah idle 65 detik | 1.332,2 MiB | 652,4 MiB | 24 → 11 |

Pengurangan working set pada 13 tab kosong sekitar **63,8%**. Private memory skenario itu turun dari **516,8 menjadi 185,8 MiB**. Pada skenario idle terakhir, tiga tab background berhasil tidur. Ketika salah satunya dipilih kembali, nilai input yang belum disimpan tetap sama, timer berlanjut, dan tidak ada reload halaman.

Working set total adalah jumlah working set proses aplikasi dan anak prosesnya. Halaman memori bersama dapat terhitung lebih dari sekali, sehingga angka ini **bukan total RAM fisik unik**. Private memory adalah private committed memory proses, bukan RAM resident. Hasil website nyata bergantung pada konten, media, jumlah tab yang telah dimuat, GPU, serta versi WebView2. Penghematan terbesar pada pengujian ini berasal dari tidak membuat renderer untuk tab kosong, bukan hanya dari menidurkan halaman. Antarmuka masih memakai satu WebView; belum diganti UI native.

## Reproduksi dan hasil mentah

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark.ps1 -Label before
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark.ps1 -Label after
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-session.ps1
```

Versi sebelum optimasi disimpan di `.cache\bench-before\AzyCore.exe`. Hasil mentah: `.cache\performance-before.json`, `.cache\performance-after.json`, `.cache\session-results.json`, dan `.cache\smoke-results.json`. Profil benchmark berada di `.cache`; endpoint debugging sementara hanya diaktifkan oleh skrip pengujian.

API lifecycle mengikuti [dokumentasi resmi WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_3). Tidak ada pengurangan sandbox atau penonaktifan pemeriksaan sertifikat untuk menghemat memori.
