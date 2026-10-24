import { expect, test, type Page } from '@playwright/test';

/** A big tile's body shows its inner folders; the tile itself is hit on its header. */
const header = { position: { x: 12, y: 10 } };

/** ⌘-select Cache JetBrains (🟢) in the overview, then Documents/Projects (🔴), and open the dialog. */
async function openWithSafeAndImportant(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: /^Cache JetBrains,/ }).click({ modifiers: ['Meta'] });
  await page.getByRole('button', { name: /^Documents,/ }).dblclick(header);
  await page
    .getByRole('button', { name: /^Projects,/ })
    .click({ modifiers: ['Meta'] });
  await page.getByRole('contentinfo').getByRole('button', { name: 'Xoá…' }).click();
  return page.getByRole('dialog');
}

test('delete: important row set to permanent needs the ack, then shows freed space', async ({ page }) => {
  const dlg = await openWithSafeAndImportant(page);
  await expect(dlg.getByRole('heading')).toHaveText('Xoá 2 mục');

  const important = dlg.getByRole('radiogroup', { name: /Projects/ });
  await expect(
    dlg.getByRole('radiogroup', { name: /JetBrains/ }).getByRole('radio', { name: 'Xoá hẳn' }),
  ).toBeChecked();
  await expect(important.getByRole('radio', { name: 'Thùng rác' })).toBeChecked();
  await expect(dlg).toContainText('Xoá hẳn: 4.0 GB · Vào Thùng rác: 20.0 GB');

  await important.getByRole('radio', { name: 'Xoá hẳn' }).click();
  const del = dlg.getByRole('button', { name: 'Xoá (1 mục quan trọng)' });
  await expect(del).toBeDisabled();
  await dlg.getByRole('checkbox', { name: 'Tôi hiểu dữ liệu này không thể khôi phục' }).check();
  await expect(del).toBeEnabled();
  await page.screenshot({ path: 'test-results/delete-dialog.png' });

  await del.click();
  const result = page.getByRole('dialog');
  await expect(result.getByRole('heading')).toHaveText('Kết quả xoá');
  await expect(result).toContainText('đã giải phóng 24.0 GB');
  await expect(page.getByRole('contentinfo')).toHaveCount(0); // selection cleared
  await result.getByRole('button', { name: 'Đóng' }).click();

  // Level reloaded: Projects is gone from Documents.
  await expect(page.getByRole('button', { name: /^Projects,/ })).toHaveCount(0);
  await expect(page.getByRole('button', { name: /^Photos,/ })).toBeVisible();

  await page.getByRole('button', { name: 'Lịch sử' }).click();
  const hist = page.getByRole('dialog');
  await expect(hist.getByRole('row')).toHaveCount(3);
  await expect(hist).toContainText('/Users/demo/Documents/Projects');
});

test('delete: a running app is reported in plain language', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: /^Cache Telegram,/ }).click();
  await page.getByRole('complementary').getByRole('button', { name: 'Xoá…' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Xoá', exact: true }).click();
  const alert = page.getByRole('dialog').getByRole('alert');
  await expect(alert).toContainText('Hãy thoát Telegram trước rồi thử lại');
  await expect(page.getByRole('dialog')).toContainText('chưa thấy dung lượng trống thêm');
});

test('delete: a tool row is cleaned with its tool, not Trash/permanent', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: /^Cache Homebrew,/ }).click();
  await expect(page.getByRole('complementary')).toContainText('Dọn bằng brew.');
  await page.getByRole('complementary').getByRole('button', { name: 'Xoá…' }).click();
  const dlg = page.getByRole('dialog');
  await expect(dlg).toContainText('Dọn bằng brew');
  await expect(dlg.getByText('brew cleanup -s')).toBeVisible();
  await expect(dlg.getByRole('radiogroup')).toHaveCount(0);
  await expect(dlg).toContainText('Dọn bằng công cụ: 3.0 GB');

  await dlg.getByRole('button', { name: 'Xoá', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Đóng' }).click();
  await page.getByRole('button', { name: 'Lịch sử' }).click();
  // A tool decides what goes: history shows no byte count, only the tool.
  const row = page.getByRole('dialog').getByRole('row').nth(1);
  await expect(row).toContainText('Dọn bằng brew');
  await expect(row).toContainText('—');
});

test('delete: removing the folder being viewed goes back up, not to its stale contents', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: /^Library,/ }).dblclick(header);
  await page.getByRole('button', { name: /^Developer,/ }).click({ modifiers: ['Meta'], ...header });
  await page.getByRole('button', { name: /^Developer,/ }).dblclick(header);
  await expect(page.getByRole('button', { name: /^CoreSimulator,/ })).toBeVisible();
  await page.getByRole('contentinfo').getByRole('button', { name: 'Xoá…' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Xoá', exact: true }).click();
  const result = page.getByRole('dialog');
  await expect(result.getByRole('heading')).toHaveText('Kết quả xoá');
  await result.getByRole('button', { name: 'Đóng' }).click();
  // Back at Library (the nearest crumb still on disk), not inside the deleted Developer.
  await expect(page.getByRole('button', { name: /^Caches,/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /^(Developer|CoreSimulator),/ })).toHaveCount(0);
});

test('delete: buttons are disabled while a scan runs', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: /^Cache JetBrains,/ }).click({ modifiers: ['Meta'] });
  await page.evaluate(() =>
    (window as unknown as { __scan: { setState(s: object): void } }).__scan.setState({ scanning: true }),
  );
  const del = page.getByRole('contentinfo').getByRole('button', { name: 'Xoá…' });
  await expect(del).toHaveAttribute('aria-disabled', 'true');
  await expect(del).toHaveAttribute('title', 'Đang quét — đợi quét xong rồi mới xoá');
  await del.click({ force: true });
  await expect(page.getByRole('dialog')).toHaveCount(0);
});
