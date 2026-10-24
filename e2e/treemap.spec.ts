import { expect, test } from '@playwright/test';

test('treemap: focus, drill down, breadcrumb back', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/');

  const telegram = page.getByRole('button', { name: /^Cache Telegram,/ });
  await expect(telegram).toBeVisible();
  const legend = page.getByRole('list', { name: 'Chú thích màu' });
  for (const name of ['An toàn', 'Cân nhắc', 'Quan trọng', 'Chưa phân loại', 'Hệ thống', 'Chưa giải thích được'])
    await expect(legend.getByText(name, { exact: true })).toBeVisible();
  await page.screenshot({ path: 'test-results/legend.png' });
  await telegram.click();
  await expect(page.getByRole('complementary')).toContainText('12.0 GB');
  await expect(page.getByRole('complementary')).toContainText('Ảnh, video đã xem');

  await page.screenshot({ path: 'test-results/treemap.png' });

  const crumbs = page.getByRole('navigation').getByRole('button');
  await expect(crumbs).toHaveCount(1);
  await page.getByRole('button', { name: /^Library,/ }).dblclick();
  await expect(crumbs).toHaveCount(2);
  await crumbs.first().click();
  await expect(crumbs).toHaveCount(1);
  await expect(telegram).toBeVisible();

  // Keyboard: Space selects, system tiles cannot be selected.
  await telegram.focus();
  await page.keyboard.press('Space');
  await expect(telegram).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('checkbox', { name: /Applications/ })).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('perf: level with 2,000 children (+ 2 nested folders of 200) renders < 100 ms', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Cache Telegram,/ })).toBeVisible();
  const { ms, nestedMs, inner, tiles } = await page.evaluate(async () => {
    const s = (window as unknown as { __scan: { getState(): { open(id: number): Promise<void> } } }).__scan;
    const frame = () => new Promise((r) => requestAnimationFrame(() => setTimeout(r, 0)));
    const t0 = performance.now();
    await s.getState().open(9000);
    await frame();
    const ms = performance.now() - t0;
    // Inner tiles arrive with a second (batched) request + redraw.
    const inner = () => document.querySelectorAll('[aria-label^="inner-"]').length;
    while (inner() === 0 && performance.now() - t0 < 5000) await frame();
    return { ms, nestedMs: performance.now() - t0, inner: inner(), tiles: document.querySelectorAll('.tile').length };
  });
  console.log(
    `perf: 2000 children -> ${tiles} tiles; first paint ${ms.toFixed(1)} ms, with nested ${nestedMs.toFixed(1)} ms (open + layout + render + paint)`,
  );
  expect(inner).toBeGreaterThan(0); // nested tiles were drawn
  expect(ms).toBeLessThan(100);
  expect(nestedMs).toBeLessThan(100);
});

test('nested: big tiles show their folders; setting turns it off; ⚫ never nests', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/');
  const docs = page.getByRole('button', { name: /^Documents,/ });
  await expect(docs).toBeVisible();
  // Projects + Photos inside the Documents tile: real names, never the shared rule name twice.
  const inner = page.getByRole('button', { name: /^(Projects|Photos),/ });
  await expect(inner).toHaveCount(2);
  await expect(page.getByRole('button', { name: /^Tài liệu cá nhân,/ })).toHaveCount(0);
  const apps = page.locator('div:has(> button[aria-label^="Applications,"])');
  await expect(apps.locator('.tile')).toHaveCount(1); // only its own tile
  await page.screenshot({ path: 'test-results/nested.png' });

  await page.getByRole('button', { name: /Cài đặt/ }).click();
  const sw = page.getByRole('switch', { name: 'Hiện thư mục con trong ô lớn' });
  await expect(sw).toBeChecked();
  await sw.click();
  await page.keyboard.press('Escape');
  await expect(inner).toHaveCount(0);
  await page.reload();
  await expect(docs).toBeVisible();
  await expect(inner).toHaveCount(0);

  await page.getByRole('button', { name: /Cài đặt/ }).click();
  await page.getByRole('switch').click();
  await page.keyboard.press('Escape');
  await expect(inner).toHaveCount(2);

  // Inner tiles act like tiles: click focuses, double-click opens.
  const crumbs = page.getByRole('navigation').getByRole('button');
  await expect(crumbs).toHaveCount(1);
  await inner.last().click();
  await expect(page.getByRole('complementary')).toContainText('15.0 GB');
  await expect(crumbs).toHaveCount(1); // the click did not reach Documents
  await inner.last().dblclick();
  await expect(crumbs).toHaveCount(3);
  await expect(crumbs.last()).toHaveText('Photos'); // real names off the overview
  // Inside Documents: real folder names, rule name only in the tooltip / details.
  await crumbs.nth(1).click();
  await expect(page.getByRole('button', { name: /^Projects,.*Tài liệu cá nhân/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /^Tài liệu cá nhân,/ })).toHaveCount(0);
  await expect(page.getByRole('button', { name: /^Vacation,/ })).toBeVisible(); // inside Photos
  await page.screenshot({ path: 'test-results/documents.png' });
  await expect(page.getByRole('button', { name: /^Family,/ })).toBeVisible(); // inside Photos
  expect(errors).toEqual([]);
});

type Store = { getState(): { open(id: number): Promise<void>; level: { node: { id: number } } | null } };

test('overlapping opens: the latest call wins', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Cache Telegram,/ })).toBeVisible();
  const id = await page.evaluate(async () => {
    const w = window as unknown as { __scan: Store; __mockDelay: Record<number, number> };
    w.__mockDelay = { 1: 200 }; // Library answers late
    const s = w.__scan.getState();
    await Promise.all([s.open(1), s.open(19)]);
    return w.__scan.getState().level?.node.id;
  });
  expect(id).toBe(19);
  await expect(page.getByRole('navigation').getByRole('button').last()).toHaveText('Documents');
});

test('cancel: no error, a new scan is not disturbed', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Cache Telegram,/ })).toBeVisible();
  await page.getByRole('button', { name: 'Quét lại' }).click();
  await page.getByRole('button', { name: 'Huỷ' }).click();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Quét lại' })).toBeEnabled();
});

test('selection bar: total counts top-level only, clear hides it', async ({ page }) => {
  await page.goto('/');
  const bar = page.getByRole('contentinfo');
  await expect(bar).toHaveCount(0);
  await page.getByRole('button', { name: /^Cache Telegram,/ }).click({ modifiers: ['Meta'] });
  await page.getByRole('button', { name: /^Cache JetBrains,/ }).click({ modifiers: ['Meta'] });
  await expect(bar).toContainText('Đã chọn 2 mục · 16.0 GB');
  await bar.getByRole('button', { name: 'Bỏ chọn' }).click();
  await expect(bar).toHaveCount(0);
});
