# PERF — báo cáo hiệu năng

Máy đo: macOS (Darwin 25.6), release mode, chưa đo app bundle. Chạy 2 lần mỗi mục.

| Chỉ số | Ngân sách | Đo được | Cách đo | Đạt? |
|---|---|---|---|---|
| Installer | ≤ 10 MB | chưa đo — cần build app (binary release = 3,4 MB, cận dưới; JS gzip 125 KB) | `ls -l src-tauri/target/release/diskviet`; `gzip -c` các file `dist/assets/*` | chưa đo — cần build app |
| RAM rảnh | ≤ 120 MB | chưa đo — cần build app | Activity Monitor | chưa đo — cần build app |
| Quét `~` (~90 GB) | ≤ 15 s | 11,9 s (lần 1, nguội) / 6,4 s, 6,2 s (ấm); 153.616 node, 87,06 GB | `/usr/bin/time -l <test bin> home_scan_timing --ignored --nocapture` | Đạt |
| RAM quét `~` | — | 70 MB peak RSS (2 lần: 70,9 / 70,3 MB) | như trên, "maximum resident set size" | — |
| Quét `/` (boot) | — | 10,5 s / 10,8 s; 431.347 node, 188,0 GB | `boot_scan_skips_system_volumes` | — |
| RAM quét ~2 triệu file | ≤ 300 MB | `/`: 1.657.913 file (431.826 node; file < 1 MB gộp vào `small_*`), peak RSS 157 MB (~95 B/file) → ngoại suy 2 triệu file ≈ 190 MB | `boot_scan_skips_system_volumes` (in số file) + `/usr/bin/time -l` | Đạt (ngoại suy từ 1,66 triệu file) |
| Vẽ / drill-down (2.000 con) | ≤ 100 ms | 42,1 ms (191 tile) | `pnpm test:e2e`, case perf trong `e2e/treemap.spec.ts` (Chromium, không phải WKWebView) | Đạt |
| JS bundle | — | index 386 kB → 120 kB gzip; CSS 17,5 kB → 4,4 kB gzip; chunk nhỏ khác < 1 kB | `pnpm build` + `gzip -c` | — |

## Đối chiếu dung lượng
- `$HOME`: scan 87.065.104.384 B = 85,02 triệu KiB; `du -sk ~` = 85.023.568 KiB. Khớp (du bị TCC chặn `~/Movies/TV`, `~/.Trash`).
- `/`: scan 188,0 GB (≈183,6 triệu KiB). `df -k`: `/` (sealed system) dùng 12,3 triệu KiB + `/System/Volumes/Data` dùng 192,0 triệu KiB = 204,4 triệu KiB. Scan thấp hơn ≈10%: phần chênh là snapshot/purgeable, vùng TCC chặn, VM/swap... Cần xem "Chưa giải thích được" trong app có bù đúng phần này không (mục dưới). Khoảng chênh ~10% này chính là ô "Chưa giải thích được" trong app (volume APFS khác, thư mục không đọc được khi thiếu Full Disk Access, snapshot).

## Cần kiểm tra thủ công
- [ ] `pnpm tauri build` → ghi dung lượng dmg (ngân sách ≤ 10 MB)
- [ ] RAM lúc rảnh trong Activity Monitor (app + WebContent) ≤ 120 MB
- [ ] RAM khi quét ổ boot (~2 triệu file nếu có) ≤ 300 MB (đo thật: 157 MB / 1,66 triệu file)
- [ ] Quét ổ boot: tổng + "Chưa giải thích được" ≈ `df` đã dùng (±1%)
- [ ] Xoá DerivedData, cache Telegram (khi Telegram đang chạy → phải hiện hộp thoại yêu cầu thoát), một Simulator runtime
- [ ] Mục trong Trash → "Put Back" của Finder hoạt động
- [ ] Mục 🔑 → hỏi mật khẩu đúng một lần
- [ ] "Trống trước/sau" khớp `df -k /`
- [ ] Giao diện light mode
