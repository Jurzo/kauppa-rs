// MB-0047 (RR-0041): Playwright with its pinned Chromium only. Started by
// scripts/browser/run.sh, which builds the app, serves it on a loopback port
// with a throwaway database and passes its address in KAUPPA_BASE_URL.
import { defineConfig, devices } from 'playwright/test';

const baseURL = process.env.KAUPPA_BASE_URL;
if (!baseURL) {
  throw new Error('KAUPPA_BASE_URL is unset: run scripts/browser/run.sh');
}

export default defineConfig({
  testDir: './flows',
  // The flows share one account and one database; run them in order.
  workers: 1,
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  // A flaky flow should show as a failure while the job's flake rate is
  // measured (RR-0041 sub-answer 3), not be retried into a pass.
  retries: 0,
  timeout: 30_000,
  expect: { timeout: 5_000 },
  reporter: process.env.CI ? [['list'], ['github']] : 'list',
  use: {
    baseURL,
    locale: 'en-US',
    trace: 'retain-on-failure',
  },
  projects: [
    { name: 'setup', testMatch: /setup\.js$/, use: { ...devices['Desktop Chrome'] } },
    {
      name: 'chromium',
      testMatch: /\.spec\.js$/,
      dependencies: ['setup'],
      use: { ...devices['Desktop Chrome'], storageState: '.auth/user.json' },
    },
  ],
});
