# Decisions

Each dependency and notable design choice, with why. Add entries when you add a dependency.

## Stack (Phase 0)
| Choice | Why |
|---|---|
| Tauri 2.x | OS WebView → small installer, low RAM vs Electron |
| React 19 + Vite 8 (`@vitejs/plugin-react`) | ecosystem; fast dev, rollup-based prod build |
| TypeScript 5.9 (not 7.x), `strict: true` | typescript-eslint does not support TS 7 yet |
| radix-ui (unified pkg) | accessible unstyled primitives (dialogs, menus), tree-shaken per primitive |
| zustand 5 | ~1 KB state lib for scan/selection stores |
| lucide-react | named imports tree-shake per icon |
| @tauri-apps/api | typed `invoke`/`listen` to Rust |
| @tauri-apps/plugin-dialog | native folder picker / confirm dialogs |
| @playwright/test | e2e smoke in mock mode (`vite --mode mock`, no Tauri needed) |
| eslint + typescript-eslint + eslint-plugin-react-hooks + @eslint/js | lint |
| prettier | formatting |
| @tauri-apps/cli | `pnpm tauri dev/build/icon` |
| @types/node | types for vite/playwright configs |
| Tailwind v4 (`tailwindcss`, `@tailwindcss/vite`) | zero runtime, small CSS; spec §3 stack; theme kept as CSS variables |

## Rust
| Choice | Why |
|---|---|
| tauri, tauri-build, tauri-plugin-dialog | app shell, build script, native dialogs |
| serde, serde_json | IPC payloads |
| thiserror 2 | typed `AppError` serialized as `{kind,message}` |
| libc | `statfs` and `getmntinfo` for real free/used bytes and the volume list |
| rayon 1 | scanner walks sibling directories in parallel (work-stealing); needed for the ≤ 15 s `~` scan budget. Hard-link set uses `std::sync::Mutex` (not `parking_lot`, avoids a direct dep) |
| trash 5 (`default-features = false`) | Thùng rác qua `DeleteMethod::Finder` (osascript) — cách duy nhất có "Put Back"; `NsFileManager` không có (spec §7). Lỗi Finder (vd. bị từ chối quyền Automation) báo `trash_failed: …`, **không** tự chuyển sang xoá hẳn. Riêng symlink dùng `NsFileManager` (Finder treo tới AppleEvent timeout với symlink trỏ tới thư mục; symlink không cần Put Back). Tắt default features: bỏ `chrono` (chỉ dùng trên Linux/freedesktop) |
| `profile.release`: `opt-level="s"`, lto, codegen-units=1, panic=abort, strip | size-first |
| `profile.dev.package."*"` opt-level 2 | dependencies (later: walkers) stay fast in dev |

## Quyết định của người lập kế hoạch (spec §9 để người xây quyết định)

| Việc | Chọn | Vì sao |
|---|---|---|
| Treemap | Tự viết squarified (TS, ~100 dòng) | Không cần thư viện |
| Duyệt thư mục | Tự viết, đệ quy song song bằng `rayon` | Cần dựng cây trực tiếp, kiểm soát `st_dev`, inode, huỷ |
| Lưu file nhỏ | File < 1 MB **không** tạo node; cộng vào `small_bytes`/`small_count` của thư mục cha, hiện thành ô "Khác" | Giữ RAM ≤ 300 MB với 2 triệu file |
| Thùng rác | crate `trash`, macOS dùng `DeleteMethod::Finder` | Cách NSFileManager **không** có "Put Back" (khác spec §7, ghi vào DECISIONS) |
| Mục 🔑 (cần admin) | Chỉ cho **Xoá hẳn** | Helper chạy bằng root sẽ đưa file vào Thùng rác của root, người dùng không thấy |
| Kiểm tra app đang chạy | `pgrep -x <tên>` | Không thêm `sysinfo` |
| Cache kết quả quét lần trước | Không làm | YAGNI |
| Test thuật toán TS | `node --test` (Node 24 chạy thẳng `.ts`) | Không thêm vitest |

