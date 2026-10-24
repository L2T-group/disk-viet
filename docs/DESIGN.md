# DiskViet — Bản thiết kế

> Ngày: 2026-10-24 · Tài liệu thiết kế ban đầu
> Ghi chú: phần đánh dấu **[đề xuất]** là đề xuất, chủ dự án chưa xác nhận — có thể đổi.

---

## 1. Mục đích và lý do

Khi ổ đĩa đầy, chủ dự án mất công ở ba chỗ:

1. **Không thấy được cái gì chiếm chỗ** — phải gõ `du`, `df`, đào từng thư mục.
2. **Không biết cái nào xoá được** — sợ xoá nhầm dữ liệu quan trọng.
3. **Ngại gõ lệnh** — `xcrun simctl`, `sudo rm -rf`, `brew cleanup`…

DiskViet là app desktop giúp **giảm dung lượng nhanh mà không xoá nhầm dữ liệu quan trọng**: nhìn thấy ngay vùng nào lớn, mỗi vùng được gắn mức an toàn kèm lời giải thích "vì sao nó lớn, xoá thì sao", và xoá bằng vài cú bấm.

Bối cảnh thật (máy chủ dự án, 2026-10-24, ổ 228 GB còn 13 GB): Simulator iOS (runtime + máy ảo + cache) ~45 GB, cache Telegram 12 GB, Cursor 8 GB, Gradle 6 GB, Android AVD 6 GB, DerivedData 4 GB… Phần lớn là **dữ liệu tự tạo lại được nhưng không bao giờ tự dọn**.

## 2. Mục tiêu / Không làm / Thất bại nếu

**Mục tiêu**
- Quét ổ đĩa và hiển thị dạng **treemap** (ô to = chiếm nhiều), màu = mức an toàn.
- Mỗi vùng đã biết có: tên dễ hiểu, mức an toàn, **vì sao lớn**, **xoá thì ảnh hưởng gì**, có tự tạo lại không.
- Xoá **một ô** hoặc **chọn nhiều ô** rồi xoá một lần.
- Mỗi mục chọn **Thùng rác** hoặc **Xoá hẳn**, mặc định theo mức an toàn.
- Đa ngôn ngữ: **Tiếng Việt và Tiếng Anh** trước, thêm ngôn ngữ dễ.
- Chạy **macOS** trước; kiến trúc sẵn sàng cho **Windows, Linux** sau.

**Không làm (bản đầu)**
- Không phát hành App Store (sandbox không cho đọc toàn ổ).
- Không chạy nền, không tự động dọn theo lịch, không cảnh báo khi ổ sắp đầy.
- Không tìm file trùng lặp, không gỡ ứng dụng kèm "rác" của nó.
- Không license, không auto-update, không telemetry.
- Không quản lý ảnh/Photos, iCloud, Time Machine.

**Thất bại nếu (dù chạy được)**
- Xoá mất dữ liệu người dùng mà họ không chủ ý xác nhận.
- Xoá thứ làm hỏng hệ thống / ứng dụng không mở được nữa.
- Báo "giải phóng 20 GB" nhưng thực tế ổ không trống thêm (con số sai làm mất niềm tin).
- Chậm hoặc nặng như các app dọn rác thương mại; RAM cao khi quét.

## 3. Ràng buộc

- **Người dùng:** chỉ chủ dự án (dev mobile/desktop: Xcode, Android Studio, Flutter, Expo, Tauri, Cursor, Homebrew). Bản đầu ưu tiên đúng các công cụ này.
- **Công nghệ:**
  - Tauri 2, lõi Rust (edition 2024) làm mọi việc nặng: quét, tính dung lượng, phân loại, xoá.
  - Frontend React 19 + TypeScript strict + Vite + Tailwind v4 + Radix UI + Zustand, `lucide-react` cho icon, pnpm.
  - i18n tự viết trong `src/i18n` (từ điển `vi`/`en` theo khoá, không thư viện).
  - `cargo clippy -- -D warnings` sạch; thêm dependency phải ghi lý do vào `docs/DECISIONS.md`.
  - Profile release (`opt-level="s"`, `lto`, `strip`, `panic="abort"`).
- **Quyền:** cần Full Disk Access để đọc Mail/Messages/MobileSync…; thao tác trong `/Library`, `/private` cần quyền admin.

### Ngân sách hiệu năng **[đề xuất]**
| Chỉ số | Mục tiêu |
|---|---|
| Installer macOS | ≤ 10 MB |
| RAM khi rảnh | ≤ 120 MB (tính cả WebView) |
| RAM khi quét toàn ổ (~2 triệu file) | ≤ 300 MB |
| Quét toàn bộ `~` (~90 GB, SSD) | ≤ 15 giây, có tiến độ trực tiếp |
| Vẽ treemap / drill-down | ≤ 100 ms mỗi lần đổi cấp |

## 4. Những gì đã quyết định

### 4.1 Màn hình chính: treemap (chủ dự án chọn phương án C)

```
┌──────────────────────────────────────────────────────────────┐
│ Macintosh HD › Toàn bộ · 175/228 GB · trống 26 GB  [Quét lại]│
├───────────────────────────────────────────┬──────────────────┤
│ ┌───────────┬──────────┬──────────┐       │ Cache Telegram   │
│ │ 🔒Cá nhân │Telegram ✓│SimCache ✓│       │ 12 GB · 🟢 An toàn│
│ │   35G     │  12G     │  11G     │       │ Ảnh/video đã xem  │
│ │           ├──────────┼──────────┤       │ được lưu lại. Xoá │
│ │           │SimDevice │ Cursor   │       │ thì tải lại khi   │
│ ├───────────┼──────────┼──────────┤       │ mở. [Xoá…]        │
│ │ Apps 27G  │Gradle 6G │Caches ✓  │       │                   │
│ └───────────┴──────────┴──────────┘       │                   │
├───────────────────────────────────────────┴──────────────────┤
│ Đã chọn 3 mục · 28 GB                    [Bỏ chọn]  [Xoá…]   │
└──────────────────────────────────────────────────────────────┘
```

- **Bấm 1 lần** vào ô → bảng chi tiết bên phải (tên, dung lượng, mức an toàn, vì sao lớn, hậu quả khi xoá, đường dẫn thật, nút "Mở trong Finder", nút Xoá riêng ô đó).
- **⌘-click hoặc tick góc ô** → thêm vào vùng chọn; thanh dưới cộng tổng số mục và dung lượng.
- **Bấm đúp** → mở vào thư mục con (drill-down); thanh breadcrumb để quay lên. **[đề xuất]**
- Màu ô theo mức an toàn (mục 5). Ô quá nhỏ gộp thành "Khác (n mục)".
- Ở cấp cao nhất, các vùng đã biết (theo danh mục quy tắc, mục 6) hiển thị như ô riêng có tên thân thiện ("Cache Telegram") thay vì đường dẫn thô; phần còn lại hiển thị theo cây thư mục.

### 4.2 Phạm vi quét **[đề xuất]**

- Thanh trên cùng có **chọn ổ đĩa** (volume): mặc định ổ khởi động (`/` + `/System/Volumes/Data` gộp làm một); chọn được ổ khác như `/Volumes/Data` (nơi chứa dự án của chủ dự án), hoặc **một thư mục bất kỳ**.
- Quét theo cây thư mục thật, sau đó áp danh mục quy tắc để đặt tên/mức an toàn. Thư mục `node_modules`/`target`/`build`/`.dart_tool`/`Pods` bên trong thư mục có `.git` được nhận diện ở bất kỳ ổ nào đang quét.

### 4.3 Xoá

- Hộp thoại xác nhận liệt kê từng mục, **mỗi dòng chọn Thùng rác / Xoá hẳn** (chủ dự án quyết định: hỏi người dùng, tuỳ mức an toàn).
- Mặc định **[đề xuất]**: 🟢 → Xoá hẳn; 🟡 → Thùng rác; 🔴 → Thùng rác; ⚪ chưa phân loại → Thùng rác.
- Mục 🔴 (chủ dự án quyết định): **vẫn cho xoá nhưng thêm một bước xác nhận**, nói rõ đây là dữ liệu quan trọng, cần xem xét kỹ, xoá hẳn sẽ không khôi phục được. Cụ thể **[đề xuất]**: muốn chọn "Xoá hẳn" cho mục 🔴 phải tick "Tôi hiểu dữ liệu này không thể khôi phục"; nút Xoá chuyển màu đỏ và ghi rõ số mục quan trọng.
- Mục cần quyền admin có biểu tượng 🔑; macOS hỏi mật khẩu một lần cho cả đợt xoá.
- Sau khi xoá: hiện **dung lượng trống thực tế trước/sau** (đo bằng `statfs`, không phải cộng dồn kích thước file) — chống thất bại "báo sai số".

### 4.4 Đa ngôn ngữ

- Từ điển `vi`/`en`; mặc định theo ngôn ngữ hệ thống, đổi được trong Cài đặt.
- **Mọi văn bản của quy tắc** (tên vùng, "vì sao lớn", "xoá thì sao") cũng là khoá i18n — Rust chỉ trả về `rule_id`, frontend dịch.
- Đơn vị dung lượng: GB thập phân (1 GB = 10⁹ byte) giống Finder. **[đề xuất]**

## 5. Mô hình an toàn

| Mức | Màu | Ý nghĩa | Cho xoá? |
|---|---|---|---|
| **An toàn** | 🟢 xanh | Cache/file build, **tự tạo lại được**, xoá không mất gì ngoài thời gian tải/build lại | Có |
| **Cân nhắc** | 🟡 vàng | Xoá được nhưng **mất tiện ích**: máy ảo, lịch sử chat AI, SDK phải tải lại nhiều GB, file đã tải về | Có |
| **Quan trọng** | 🔴 đỏ | **Dữ liệu của người dùng**: tài liệu, ảnh, dự án, tin nhắn, bản sao lưu iPhone | Có, thêm bước xác nhận |
| **Hệ thống** | ⚫ xám đậm, 🔒 | Hệ điều hành, ứng dụng, cấu hình — xoá có thể làm hỏng máy hoặc app | **Không** (chỉ xem) |
| **Chưa phân loại** | ⚪ xám nhạt | Không khớp quy tắc nào | Có, xử lý như 🔴 **[đề xuất]** |

**Hàng rào bảo vệ trong Rust (không phụ thuộc giao diện)** — mọi lệnh xoá đi qua một hàm kiểm tra duy nhất:
1. `canonicalize` đường dẫn; **không đi theo symlink** (xoá link chứ không xoá đích).
2. **Từ chối tuyệt đối** (danh sách cứng trong mã): `/`, `/System`, `/usr` (trừ `/usr/local`), `/bin`, `/sbin`, `/private/var/db`, `/Library/Apple`, thư mục gốc `~`, `~/Library` (bản thân nó), `~/Documents`, `~/Desktop`, `~/Pictures` (bản thân thư mục, không phải nội dung con), mọi volume gốc `/Volumes/*`.
3. Không cho xoá app đang chạy hoặc dữ liệu của app đang chạy → nhắc "Hãy thoát Telegram trước" (kiểm tra theo danh sách tiến trình của quy tắc).
4. Mức ⚫ không bao giờ xoá được, kể cả qua API nội bộ.
5. Ghi **nhật ký xoá** (thời gian, đường dẫn, kích thước, Thùng rác/xoá hẳn) vào `~/Library/Application Support/DiskViet/history.jsonl`; màn hình "Lịch sử" đọc lại được.

## 6. Danh mục quy tắc (macOS) — nơi chiếm nhiều, mức an toàn

Mỗi quy tắc gồm: `id`, mẫu đường dẫn, mức an toàn, có cần admin không, tiến trình cần thoát, **cách xoá** (xoá file hay gọi công cụ chính chủ), khoá i18n cho tên/lý do/hậu quả. Danh mục là dữ liệu tĩnh trong Rust (một file mỗi hệ điều hành).

### 🟢 An toàn để xoá
| Vùng | Đường dẫn | Vì sao lớn | Xoá thì |
|---|---|---|---|
| Xcode DerivedData | `~/Library/Developer/Xcode/DerivedData` | File build trung gian mọi dự án | Build lần đầu chậm hơn |
| Cache Simulator | `/Library/Developer/CoreSimulator/Caches` 🔑 | Cache dyld/runtime | Tự tạo lại |
| iOS DeviceSupport | `~/Library/Developer/Xcode/iOS DeviceSupport/*` | Symbol cho mỗi iOS từng cắm máy | Tải lại khi cắm máy |
| Cache Telegram | `~/Library/Caches/ru.keepcoder.Telegram`, `~/Library/Group Containers/*.ru.keepcoder.Telegram/**/postbox/media` | Media đã xem lưu mãi | Tải lại khi mở · cần thoát Telegram |
| Cache trình duyệt | `~/Library/Caches/Google`, `…/com.apple.Safari`, … | Cache web | Trang tải chậm hơn lần đầu |
| Cache JetBrains | `~/Library/Caches/JetBrains` | Index IDE | Index lại vài phút |
| Cursor/VS Code cache | `~/Library/Application Support/{Cursor,Code}/{Cache,CachedData,CachedExtensionVSIXs,GPUCache}` | Cache Electron | Tự tạo lại · cần thoát app |
| Homebrew | gọi `brew cleanup -s` | File tải và phiên bản cũ | Không ảnh hưởng gói đang cài |
| npm / pnpm / yarn | `~/.npm/_cacache`, `~/Library/Caches/pnpm`, `~/Library/pnpm/store` | Gói đã tải | Lần install sau tải lại |
| pip / uv | `~/Library/Caches/pip`, `~/.cache/uv` | Gói Python đã tải | Như trên |
| CocoaPods cache | `~/Library/Caches/CocoaPods` | Pod đã tải | Như trên |
| Log | `~/Library/Logs/*` | Log ứng dụng | Không |

### 🟡 Cân nhắc
| Vùng | Đường dẫn / cách xoá | Vì sao lớn | Xoá thì |
|---|---|---|---|
| Runtime Simulator không dùng | `xcrun simctl runtime list -v` → `xcrun simctl runtime delete <id>` | Mỗi bản iOS ~8 GB, nâng Xcode không xoá bản cũ | Muốn test iOS đó phải tải lại |
| Máy ảo Simulator | `~/Library/Developer/CoreSimulator/Devices` → `xcrun simctl erase` / `delete unavailable` | App + dữ liệu cài trong simulator | Mất app/dữ liệu test |
| Gradle caches | `~/.gradle/caches`, `~/.gradle/wrapper/dists` | Thư viện Android/Java | Build sau tải lại vài GB |
| Android AVD | `~/.android/avd/*.avd` | Ổ ảo emulator | Mất emulator đó |
| Android SDK thành phần | `~/Library/Android/sdk/{system-images,ndk,…}` | SDK nhiều phiên bản | Tải lại qua SDK Manager |
| Flutter/Dart | `~/.pub-cache`, `~/.dartServer` | Gói Dart | `flutter pub get` tải lại |
| Expo | `~/.expo` | Cache Expo | Tải lại |
| `node_modules` / `target` / `build` trong dự án | quét thư mục dự án | Phụ thuộc/build của từng dự án | Phải `install`/build lại dự án đó |
| Cursor globalStorage | `~/Library/Application Support/Cursor/User/globalStorage` | Lịch sử chat AI, trạng thái extension | **Mất lịch sử chat** |
| Docker | gọi `docker system df` / `prune` (nếu có Docker) | Image, volume, build cache | Mất image/container dừng |
| Cập nhật macOS đã tải | `/Library/Updates` 🔑 | Bản cập nhật chưa cài | Phải tải lại |
| Downloads | `~/Downloads` | File đã tải | Có thể là file cần giữ |
| Xcode Archives | `~/Library/Developer/Xcode/Archives` | Mỗi lần Archive giữ bản build + dSYM | Mất dSYM để giải mã crash của bản đã phát hành |
| Thùng rác | `~/.Trash` (chỉ cho Xoá hẳn) | File đã xoá chưa dọn | Không khôi phục được nữa |
| Zalo/Telegram dữ liệu chat | `~/Library/Application Support/ZaloData`, … | File/ảnh trong hội thoại | Mất file cũ trong chat |

### 🔴 Quan trọng (cho xoá, thêm bước xác nhận)
| Vùng | Đường dẫn |
|---|---|
| Tài liệu, Desktop, Ảnh, Phim, Nhạc | `~/Documents`, `~/Desktop`, `~/Pictures`, `~/Movies`, `~/Music` (nội dung bên trong) |
| Thư viện Ảnh | `~/Pictures/*.photoslibrary` |
| Sao lưu iPhone/iPad | `~/Library/Application Support/MobileSync/Backup` (cần Full Disk Access) |
| Mail, Tin nhắn | `~/Library/Mail`, `~/Library/Messages` |
| Mã nguồn dự án | thư mục có `.git` (trừ `node_modules`/`target`/`build` bên trong) |
| Cấu hình dev | `~/.gitconfig`, `~/.zshrc`, `~/.config` |

### ⚫ Hệ thống (không cho xoá, chỉ hiển thị)
`/System`, `/usr` (trừ `/usr/local`), `/bin`, `/sbin`, `/private/var/vm` (swap — do macOS quản lý), `/private/var/db`, ứng dụng trong `/Applications` (bản đầu chỉ hiển thị dung lượng; gỡ app là việc của Finder), `~/Library/Keychains`, `~/.ssh`, `~/.gnupg`, `~/Library/Preferences`, volume Preboot/Recovery/VM.

### Không đếm được
Thư mục bị macOS chặn (Operation not permitted) hiện thành ô có viền nét đứt **"Không đọc được — cần Full Disk Access"** kèm nút mở System Settings đúng trang. Dung lượng "chưa giải thích" = dung lượng đã dùng của volume (`statfs`) − tổng đã quét, hiện thành một ô riêng để con số luôn khớp với ổ đĩa.

## 7. Kiến trúc

```
React (treemap, chọn, hộp thoại, i18n)
   │ invoke / event (tiến độ quét)
Rust core
   ├─ scanner   : duyệt song song, tính dung lượng, cây kết quả
   ├─ rules     : danh mục quy tắc theo OS → gắn rule_id + mức an toàn
   ├─ guard     : hàng rào an toàn (mục 5) — mọi đường xoá đều qua đây
   ├─ cleaner   : Thùng rác / xoá hẳn / gọi công cụ chính chủ / leo quyền admin
   ├─ disk      : statfs trước/sau, danh sách volume
   └─ history   : nhật ký xoá
```

**Điểm kỹ thuật bắt buộc đúng (để con số không sai):**
- Dung lượng = **block thực cấp phát** (`st_blocks × 512`), không phải `st_size` (file thưa, file nén APFS).
- Hard link: đếm mỗi inode **một lần**.
- **Không đi qua mount point** (dùng `st_dev`), không theo symlink; xử lý firmlink `/` ↔ `/System/Volumes/Data` để không đếm đôi.
- APFS clone/snapshot: xoá file clone có thể không giải phóng chỗ → luôn báo **dung lượng trống thực tế đo trước/sau**, và ghi chú khi số đo nhỏ hơn dự kiến.
- Quét có thể **huỷ giữa chừng**; gửi tiến độ cho UI theo nhịp (≤ 10 lần/giây).
- Kết quả quét giữ ở Rust; frontend chỉ nhận **cấp đang xem** (con trực tiếp + tổng), không gửi cả cây 2 triệu nút qua IPC.

**Xoá:**
- Thùng rác: API hệ thống (`NSFileManager trashItemAtURL` trên macOS — để "Put Back" hoạt động).
- Xoá hẳn: `remove_dir_all` không theo symlink.
- Mục 🔑: một lần leo quyền cho cả đợt (macOS: `osascript … with administrator privileges` gọi chính binary DiskViet ở chế độ helper với danh sách đường dẫn đã qua guard; helper **chạy lại guard** trước khi xoá). Không bao giờ ghép chuỗi đường dẫn vào lệnh shell.
- Quy tắc có "cách xoá = công cụ" (`xcrun simctl`, `brew`, `docker`): gọi trực tiếp bằng `std::process::Command` với tham số tách rời, chỉ khi công cụ có mặt.

**Đa hệ điều hành sau này:** `rules/{macos,windows,linux}.rs`, phần guard/disk/trash tách theo `cfg(target_os)`; giao diện dùng chung.

## 8. Kiểm thử

- Rust unit test cho **guard** (đây là phần quan trọng nhất): từng đường dẫn cấm, symlink trỏ ra ngoài, đường dẫn có `..`, mức ⚫.
- Rust test cho **scanner** trên cây thư mục tạm: hard link, symlink, file thưa, quyền bị từ chối.
- Test **rules**: đường dẫn mẫu → đúng `rule_id` và mức.
- Frontend chế độ `mock` với dữ liệu giả để làm UI và e2e Playwright không đụng ổ thật.
- Kiểm thử thủ công cuối: dọn thật trên máy chủ dự án, so số trước/sau với `df`.

## 9. Để người xây quyết định

- Thuật toán treemap (squarified là lựa chọn hiển nhiên) và có dùng thư viện hay tự viết (~150 dòng).
- Crate duyệt thư mục (`jwalk` hay tự viết bằng `rayon`), crate Thùng rác (`trash` hay gọi thẳng API).
- Chi tiết bố cục, màu sắc cụ thể, phím tắt, animation.
- Cấu trúc thư mục mã, tên khoá i18n, định dạng nhật ký.
- Có lưu kết quả quét lần trước để mở app hiện ngay không.
