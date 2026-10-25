export const dict = {
  vi: {
    'delete.title': 'Xoá {n} mục',
    'delete.intro': 'Chọn cách xoá cho từng mục. Thùng rác khôi phục được; Xoá hẳn thì không.',
    'delete.modeFor': 'Cách xoá {name}',
    'delete.trash': 'Thùng rác',
    'delete.permanent': 'Xoá hẳn',
    'delete.lockedAdmin': 'Cần quyền quản trị — chỉ xoá hẳn được',
    'delete.lockedTrash': 'Đã ở trong Thùng rác — chỉ xoá hẳn được',
    'delete.merged': '{n} mục nằm trong mục khác đã được gộp',
    'delete.ack': 'Tôi hiểu dữ liệu này không thể khôi phục',
    'delete.riskyNote': 'Có dữ liệu quan trọng hoặc chưa phân loại sẽ bị xoá hẳn. Hãy xem kỹ từng dòng.',
    'delete.summary': 'Xoá hẳn: {permanent} · Vào Thùng rác: {trash} (chỉ trống chỗ khi dọn Thùng rác)',
    'delete.byTool': 'Dọn bằng {tool}',
    'delete.summaryTool': 'Dọn bằng công cụ: {size}',
    'delete.selectedRuntime': '(runtime đã chọn)',
    'delete.run': 'Xoá',
    'delete.runRisky': 'Xoá ({n} mục quan trọng)',
    'delete.running': 'Đang xoá… (có thể mất vài phút nếu macOS hỏi quyền điều khiển Finder)',
    'delete.failed': 'Không xoá được',

    'delete.result.title': 'Kết quả xoá',
    'delete.result.freed': 'Trống trước {before} → sau {after} (đã giải phóng {freed})',
    'delete.result.none': 'Trống trước {before} → sau {after} (chưa thấy dung lượng trống thêm)',
    'delete.result.apfs':
      'Ít hơn dự kiến ({expected}): macOS có thể còn giữ dữ liệu trong bản chụp APFS (Time Machine cục bộ) hoặc file nhân bản (clone) dùng chung, nên chỗ trống sẽ tăng dần sau.',
    'delete.result.emptyTrash': 'Dọn Thùng rác để lấy lại {size}',
    'delete.result.ok': 'Đã xoá {n} mục',
    'delete.result.errors': '{n} mục không xoá được:',

    'delete.err.forbidden_root': 'Thư mục được bảo vệ, DiskViet không bao giờ xoá.',
    'delete.err.mount_root': 'Đây là gốc của một ổ đĩa, không xoá được.',
    'delete.err.system_path': 'Nằm trong vùng hệ thống macOS, không được xoá.',
    'delete.err.system_safety': 'Mục hệ thống, không được xoá.',
    'delete.err.holds_system':
      'Bên trong có mục hệ thống nên không xoá cả thư mục được — hãy chọn từng mục bên trong.',
    'delete.err.in_use_unknown':
      'Không kiểm tra được app nào đang dùng các file này nên chưa xoá. Hãy thử lại.',
    'delete.err.needs_ack': 'Cần tick "Tôi hiểu dữ liệu này không thể khôi phục" rồi thử lại.',
    'delete.err.admin_trash': 'Mục cần quyền quản trị không vào Thùng rác được — chọn Xoá hẳn.',
    'delete.err.trash_permanent_only': 'Mục đã ở trong Thùng rác — chọn Xoá hẳn.',
    'delete.err.use_tool': 'Mục này phải dọn bằng công cụ riêng của nó.',
    'delete.err.not_tool': 'Mục này không dọn bằng công cụ được.',
    'delete.err.invalid_path': 'Đường dẫn không hợp lệ.',
    'delete.err.not_found': 'Không còn tồn tại — có thể đã bị xoá. Hãy quét lại.',
    'delete.err.relative': 'Đường dẫn không hợp lệ (không phải đường dẫn đầy đủ).',
    'delete.err.no_home': 'Không xác định được thư mục người dùng.',
    'delete.err.quit': 'Đang được dùng bởi {app} — hãy thoát app đó (hoặc đóng chương trình đang dùng mục này) rồi thử lại.',
    'delete.err.needs_admin': 'Cần quyền quản trị — hãy chọn Xoá hẳn, macOS sẽ hỏi mật khẩu.',
    'delete.err.admin_cancelled': 'Bạn đã huỷ nhập mật khẩu — chưa xoá gì.',
    'delete.err.admin_failed':
      'Không rõ kết quả xoá bằng quyền quản trị — có thể đã xoá một phần. Hãy quét lại để xem.',
    'delete.err.not_admin_item': 'Mục này không cần quyền quản trị nên không được xoá bằng quyền đó.',
    'delete.err.tool_root_only':
      'Chỉ dọn được cả thư mục bằng công cụ của nó, không dọn từng phần bên trong.',
    'delete.err.tool_missing': 'Cần cài {tool} để dọn mục này.',
    'delete.err.tool_failed': 'Công cụ báo lỗi: {detail}',
    'delete.err.covered_by': 'Bỏ qua vì mục chứa nó ({path}) không xoá được.',
    'delete.err.no_trash_on_volume': 'Ổ này không có Thùng rác — chọn Xoá hẳn nếu chắc chắn.',
    'delete.err.trash_failed':
      'Không chuyển vào Thùng rác được. macOS có thể hỏi cho DiskViet điều khiển Finder — hãy cho phép trong Cài đặt hệ thống → Quyền riêng tư & Bảo mật → Tự động hoá, rồi thử lại.',
    'delete.err.trash_denied':
      'macOS chặn DiskViet điều khiển Finder nên không chuyển vào Thùng rác được. Mở Cài đặt hệ thống → Quyền riêng tư & Bảo mật → Tự động hoá → DiskViet, bật Finder, rồi thử lại.',
    'delete.err.mount_inside': 'Bên trong có một ổ đĩa khác được gắn vào — phần đó được giữ nguyên.',
  },
  en: {
    'delete.title': 'Delete {n} items',
    'delete.intro': 'Choose how to delete each item. The Trash can be undone; Delete permanently cannot.',
    'delete.modeFor': 'How to delete {name}',
    'delete.trash': 'Trash',
    'delete.permanent': 'Delete permanently',
    'delete.lockedAdmin': 'Needs administrator rights — can only be deleted permanently',
    'delete.lockedTrash': 'Already in the Trash — can only be deleted permanently',
    'delete.merged': '{n} items inside another selected item were merged',
    'delete.ack': 'I understand this data cannot be recovered',
    'delete.riskyNote':
      'Important or unclassified data will be deleted permanently. Check each row carefully.',
    'delete.summary':
      'Delete permanently: {permanent} · To Trash: {trash} (space is freed only when the Trash is emptied)',
    'delete.byTool': 'Cleaned with {tool}',
    'delete.summaryTool': 'Cleaned with tools: {size}',
    'delete.selectedRuntime': '(the selected runtime)',
    'delete.run': 'Delete',
    'delete.runRisky': 'Delete ({n} important)',
    'delete.running': 'Deleting… (may take a few minutes if macOS asks to let DiskViet control Finder)',
    'delete.failed': 'Could not delete',

    'delete.result.title': 'Delete result',
    'delete.result.freed': 'Free before {before} → after {after} ({freed} freed)',
    'delete.result.none': 'Free before {before} → after {after} (no extra free space yet)',
    'delete.result.apfs':
      'Less than expected ({expected}): macOS may still keep the data in APFS snapshots (local Time Machine) or shared clone files, so free space may grow later.',
    'delete.result.emptyTrash': 'Empty the Trash to get back {size}',
    'delete.result.ok': '{n} items deleted',
    'delete.result.errors': '{n} items could not be deleted:',

    'delete.err.forbidden_root': 'Protected folder; DiskViet never deletes it.',
    'delete.err.mount_root': 'This is the root of a disk and cannot be deleted.',
    'delete.err.system_path': 'Inside a macOS system area; not allowed.',
    'delete.err.system_safety': 'System item; not allowed.',
    'delete.err.holds_system':
      'It holds system items, so the whole folder cannot be deleted — choose the items inside instead.',
    'delete.err.in_use_unknown':
      'Could not check which apps are using these files, so nothing was deleted. Try again.',
    'delete.err.needs_ack': 'Tick "I understand this data cannot be recovered" and try again.',
    'delete.err.admin_trash':
      'Items needing administrator rights cannot go to the Trash — choose Delete permanently.',
    'delete.err.trash_permanent_only': 'Already in the Trash — choose Delete permanently.',
    'delete.err.use_tool': 'This must be cleaned with its own tool.',
    'delete.err.not_tool': 'This item is not cleaned with a tool.',
    'delete.err.invalid_path': 'Invalid path.',
    'delete.err.not_found': 'No longer exists — it may already be gone. Rescan.',
    'delete.err.relative': 'Invalid path (not a full path).',
    'delete.err.no_home': 'Could not find your home folder.',
    'delete.err.quit': 'In use by {app} — quit that app (or close whatever is using this item), then try again.',
    'delete.err.needs_admin':
      'Needs administrator rights — choose Delete permanently and macOS will ask for your password.',
    'delete.err.admin_cancelled': 'You cancelled the password prompt — nothing was deleted.',
    'delete.err.admin_failed':
      'The result of the administrator delete is unknown — some items may be gone. Rescan to check.',
    'delete.err.not_admin_item':
      'This item does not need administrator rights, so it was not deleted with them.',
    'delete.err.tool_root_only': 'Only the whole folder can be cleaned, with its tool; not parts inside it.',
    'delete.err.tool_missing': 'Install {tool} to clean this item.',
    'delete.err.tool_failed': 'The tool reported an error: {detail}',
    'delete.err.covered_by': 'Skipped because the item containing it ({path}) could not be deleted.',
    'delete.err.no_trash_on_volume': 'This disk has no Trash — choose Delete permanently if you are sure.',
    'delete.err.trash_failed':
      'Could not move to the Trash. macOS may ask to let DiskViet control Finder — allow it in System Settings → Privacy & Security → Automation, then try again.',
    'delete.err.trash_denied':
      'macOS blocked DiskViet from controlling Finder, so nothing went to the Trash. Open System Settings → Privacy & Security → Automation → DiskViet, turn on Finder, then try again.',
    'delete.err.mount_inside': 'Another disk is mounted inside; that part was kept.',
  },
};
