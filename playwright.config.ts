import { defineConfig, devices } from '@playwright/test';

// UI runs in a plain browser with mock IPC (`vite --mode mock`) on a dedicated
// port so tests never attach to a `pnpm dev` session; override with DV_E2E_PORT.
const CI = !!process.env.CI;
const PORT = Number(process.env.DV_E2E_PORT ?? 1430);
const baseURL = `http://localhost:${PORT}`;

export default defineConfig({
  testDir: 'e2e',
  timeout: 30_000,
  retries: CI ? 1 : 0,
  forbidOnly: CI,
  reporter: [['list']],
  use: { baseURL, viewport: { width: 1200, height: 780 }, locale: 'vi-VN', colorScheme: 'dark' },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 1200, height: 780 } } }],
  webServer: {
    command: `pnpm vite --mode mock --port ${PORT} --strictPort`,
    url: baseURL,
    reuseExistingServer: !CI,
    timeout: 60_000,
  },
});
