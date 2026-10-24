import { expect, test } from '@playwright/test';

test('language setting switches to English and persists', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: /Cài đặt|Settings/ }).click();
  await page.getByRole('radio', { name: 'English' }).click();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: 'Rescan' })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Rescan' })).toBeVisible();
});
