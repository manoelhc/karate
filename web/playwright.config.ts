import { defineConfig } from '@playwright/test';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
const cachedBrowser = `${homedir()}/Library/Caches/ms-playwright/chromium_headless_shell-1243/chrome-headless-shell-mac-arm64/chrome-headless-shell`;
const origin = process.env.KARATE_TEST_WEB_ORIGIN;
const spki = process.env.KARATE_TEST_SPKI;
if (!origin || !spki)
  throw new Error('Run make web-e2e with the private fixture.');
export default defineConfig({
  testDir: './tests',
  testMatch: '**/*.e2e.ts',
  workers: 1,
  retries: 0,
  timeout: 60000,
  use: {
    baseURL: origin,
    locale: 'en-US',
    viewport: { width: 1440, height: 1000 },
    launchOptions: {
      ...(existsSync(cachedBrowser) ? { executablePath: cachedBrowser } : {}),
      args: [`--ignore-certificate-errors-spki-list=${spki}`],
    },
    trace: 'off',
    video: 'off',
    screenshot: 'off',
  },
  reporter: 'list',
});
