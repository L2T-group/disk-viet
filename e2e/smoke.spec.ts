import { expect, test } from '@playwright/test';

test('app shell renders in mock mode', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/');
  await expect(page.getByText('DiskViet')).toBeVisible();
  expect(errors).toEqual([]);
});
