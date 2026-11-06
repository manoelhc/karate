import { test, expect, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
const owner = '00000000-0000-4000-8000-000000000091';
const viewer = '00000000-0000-4000-8000-000000000092';
const evidence = '../docs/specker/runs/2026-11-04-karate/evidence/erasure';
function sql(command: string) {
  execFileSync('psql', ['-X', '-v', 'ON_ERROR_STOP=1', '-c', command], {
    stdio: 'pipe',
  });
}
async function signin(page: Page, id: string) {
  await page.goto('/');
  await page.getByLabel('Workspace ID', { exact: true }).fill(id);
  await page.getByRole('button', { name: 'Continue to sign in' }).click();
  await expect(
    page.getByRole('heading', { name: 'Settings & access' }),
  ).toBeVisible();
  await expect(page).toHaveURL('/');
}
async function a11y(page: Page) {
  expect(
    (
      await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa'])
        .analyze()
    ).violations,
  ).toEqual([]);
}
async function expectFits(page: Page) {
  const width = await page.evaluate(() => ({
    viewport: innerWidth,
    document: document.documentElement.scrollWidth,
    overflowing: Array.from(document.querySelectorAll('body *'))
      .filter(
        (element) =>
          getComputedStyle(element).position !== 'absolute' &&
          element.getBoundingClientRect().right > innerWidth,
      )
      .map((element) => ({
        tag: element.tagName,
        class: element.className,
        right: element.getBoundingClientRect().right,
      })),
  }));
  expect(width.document, JSON.stringify(width)).toBeLessThanOrEqual(
    width.viewport,
  );
}
test('real HTTPS code/PKCE, owner recovery, localization, access and cookie boundaries', async ({
  page,
  context,
}) => {
  mkdirSync(evidence, { recursive: true });
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.addInitScript(() => {
    const metrics = { lcp: 0, cls: 0 };
    Object.defineProperty(window, '__karateLab', { value: metrics });
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) metrics.lcp = entry.startTime;
    }).observe({ type: 'largest-contentful-paint', buffered: true });
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        const shift = entry as PerformanceEntry & {
          hadRecentInput: boolean;
          value: number;
        };
        if (!shift.hadRecentInput) metrics.cls += shift.value;
      }
    }).observe({ type: 'layout-shift', buffered: true });
  });
  await page.goto('/');
  await expect(page.getByLabel('Workspace ID', { exact: true })).toBeVisible();
  // Input can finalize LCP before Chromium has delivered the first paint entry.
  // Observe an actual paint before the keyboard journey; never accept a zero sample.
  await page.waitForFunction(
    () =>
      (window as unknown as { __karateLab: { lcp: number } }).__karateLab.lcp >
      0,
  );
  await page.keyboard.press('Tab');
  await expect(
    page.getByRole('link', { name: 'Skip to workspace' }),
  ).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.locator('main')).toBeFocused();
  await a11y(page);
  await page.screenshot({
    path: `${evidence}/signin-desktop.png`,
    fullPage: true,
  });
  const lab = await page.evaluate(
    () =>
      (window as unknown as { __karateLab: { lcp: number; cls: number } })
        .__karateLab,
  );
  expect(lab.lcp).toBeGreaterThan(0);
  expect(lab.lcp).toBeLessThanOrEqual(2500);
  expect(lab.cls).toBeLessThanOrEqual(0.1);
  await page.setViewportSize({ width: 3840, height: 2160 });
  await expectFits(page);
  await page.setViewportSize({ width: 320, height: 900 });
  await a11y(page);
  await expectFits(page);
  await page.screenshot({
    path: `${evidence}/signin-mobile.png`,
    fullPage: true,
  });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await signin(page, owner);
  await expect(
    page.getByRole('heading', { name: 'Garden team' }),
  ).toBeVisible();
  const cookie = (await context.cookies()).find(
    (c) => c.name === '__Host-karate-session',
  );
  expect(cookie).toMatchObject({
    httpOnly: true,
    secure: true,
    sameSite: 'Lax',
    path: '/',
  });
  expect(await page.evaluate(() => document.cookie)).not.toContain(
    '__Host-karate',
  );
  await a11y(page);
  await page.screenshot({
    path: `${evidence}/workspace-desktop.png`,
    fullPage: true,
  });
  expect(
    await page.evaluate(
      async (id) =>
        (
          await fetch(`/v1/tenants/${id}`, {
            method: 'PATCH',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({
              name: 'Forged',
              state: 'active',
              expected_revision: 1,
            }),
          })
        ).status,
      owner,
    ),
  ).toBe(403);
  await page.getByLabel('Workspace name').fill('Orchard team');
  await page.getByRole('button', { name: 'Save name' }).click();
  await expect(
    page.getByRole('heading', { name: 'Orchard team' }),
  ).toBeVisible();
  // Concurrent change is observed through real SQL and causes the service's CAS conflict.
  sql(
    `UPDATE karate.tenants SET name='Newer team',revision=revision+1 WHERE tenant_id='${owner}'`,
  );
  await page.getByLabel('Workspace name').fill('Stale edit');
  await page.getByRole('button', { name: 'Save name' }).click();
  await expect(page.getByRole('alert')).toContainText('workspace changed');
  await page.getByRole('button', { name: 'Refresh workspace' }).click();
  await expect(page.getByRole('heading', { name: 'Newer team' })).toBeVisible();
  await page.getByRole('button', { name: 'Suspend workspace' }).click();
  await a11y(page);
  await page.keyboard.press('Escape');
  await expect(
    page.getByRole('button', { name: 'Suspend workspace' }),
  ).toBeFocused();
  await page.getByRole('button', { name: 'Suspend workspace' }).click();
  await page.getByRole('button', { name: 'Confirm change' }).click();
  await expect(page.getByText('Suspended', { exact: true })).toBeVisible();
  expect(
    await page.evaluate(
      async (id) => (await fetch(`/v1/tenants/${id}/projects`)).status,
      owner,
    ),
  ).toBe(403);
  await page.getByRole('button', { name: 'Sign out', exact: true }).click();
  await signin(page, owner);
  await expect(page.getByText('Suspended', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Resume workspace' }).click();
  await page.getByRole('button', { name: 'Confirm change' }).click();
  await expect(page.getByText('Active', { exact: true })).toBeVisible();
  await page.getByLabel('Confirm workspace ID').fill(owner);
  await page.getByRole('button', { name: 'Review deletion' }).click();
  await a11y(page);
  await page.keyboard.press('Escape');
  await expect(
    page.getByRole('button', { name: 'Review deletion' }),
  ).toBeFocused();
  await page.getByRole('button', { name: 'Review deletion' }).click();
  await page.getByRole('button', { name: 'Schedule deletion' }).click();
  await expect(page.getByText(/Deletion is scheduled/)).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Resume workspace' }),
  ).toBeDisabled();
  await page.reload();
  await expect(page.getByText(/Deletion is scheduled/)).toBeVisible();
  await page.getByLabel('Language').selectOption('pt-BR');
  await expect(
    page.getByRole('heading', { name: 'Configurações e acesso' }),
  ).toBeVisible();
  await page.setViewportSize({ width: 320, height: 900 });
  await a11y(page);
  await expectFits(page);
  await page.screenshot({
    path: `${evidence}/workspace-mobile-pt.png`,
    fullPage: true,
  });
  await page.getByLabel('Idioma').selectOption('en');
  await page.getByRole('button', { name: 'Cancel deletion' }).click();
  await expect(page.getByText(/Deletion was cancelled/)).toBeVisible();
  await expect(page.getByText('Suspended', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Resume workspace' }).click();
  await page.getByRole('button', { name: 'Confirm change' }).click();
  await expect(page.getByText('Active', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Sign out all my sessions' }).click();
  await signin(page, viewer);
  await expect(
    page.getByText('Only a workspace owner can change these settings.'),
  ).toBeVisible();
  expect(await page.getByLabel('Workspace name').count()).toBe(0);
  expect(
    await page.evaluate(
      async (id) =>
        (
          await fetch(`/v1/tenants/${id}`, {
            method: 'PATCH',
            headers: {
              'content-type': 'application/json',
              'X-Karate-CSRF': '1',
            },
            body: JSON.stringify({
              name: 'Forged',
              state: 'active',
              expected_revision: 1,
            }),
          })
        ).status,
      viewer,
    ),
  ).toBe(403);
  sql(
    `UPDATE karate.memberships SET revoked_at=clock_timestamp(), revision=revision+1 WHERE tenant_id='${viewer}'`,
  );
  await page.reload();
  await expect(page.getByRole('alert')).toContainText('membership');
  expect(await page.getByText('Read-only team').count()).toBe(0);
  await context.clearCookies();
  await page.goto('/v1/auth/callback?state=bad&error=private');
  await expect(page.getByRole('alert')).toContainText('Sign-in could not');
  await expect(page).toHaveURL('/');
  // Only this disposable, synthetic tenant is eligible early. The actual Rust
  // worker runs under its restricted role; no runtime clock/retention bypass.
  await signin(page, owner);
  await page.getByLabel('Confirm workspace ID').fill(owner);
  await page.getByRole('button', { name: 'Review deletion' }).click();
  await page.screenshot({
    path: `${evidence}/deletion-confirmation-mobile.png`,
    fullPage: true,
  });
  await a11y(page);
  await page.getByRole('button', { name: 'Schedule deletion' }).click();
  await expect(page.getByText(/Deletion is scheduled/)).toBeVisible();
  sql(
    `UPDATE karate.tenant_deletions SET erase_after=0 WHERE tenant_id='${owner}'`,
  );
  let erased = false;
  let batches = 0;
  for (; batches < 40; batches++) {
    const result = JSON.parse(
      execFileSync('../target/debug/karate-erasure', [], {
        env: {
          ...process.env,
          KARATE_ERASURE_DATABASE_URL: process.env.KARATE_TEST_ERASURE_URL,
        },
        encoding: 'utf8',
      }),
    ) as { outcome: string; deleted_rows: number };
    expect(result.deleted_rows).toBeLessThanOrEqual(100);
    if (result.outcome === 'primary_database_erased') {
      erased = true;
      break;
    }
    expect(result.outcome).toBe('progress');
  }
  expect(erased).toBe(true);
  sql(`DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM karate.tenants WHERE tenant_id='${owner}')
      OR NOT EXISTS(SELECT 1 FROM karate.erasure_receipts WHERE tenant_id='${owner}')
      OR NOT EXISTS(SELECT 1 FROM karate.tenants WHERE tenant_id='${viewer}')
    THEN RAISE EXCEPTION 'erasure isolation or receipt failed'; END IF;
  END $$`);
  await page.reload();
  await expect(page.getByLabel('Workspace ID', { exact: true })).toBeVisible();
  expect(await page.getByText('Newer team').count()).toBe(0);
  expect(errors).toEqual([]);
  writeFileSync(
    `${evidence}/browser.json`,
    JSON.stringify(
      {
        browser: context.browser()?.version(),
        realServices: ['Rust API', 'PostgreSQL', 'Dex v2.46.1', 'HTTPS proxy'],
        axeViolations: 0,
        viewportWidths: [320, 1440, 3840],
        localLab: lab,
        fieldCoreWebVitals:
          'Not measured; INP and field distribution remain open',
        screenshots: 5,
        erasure: {
          primaryDatabaseErased: erased,
          batches: batches + 1,
          foreignTenantPreserved: true,
        },
        provider:
          'synthetic connector; human-provider and screen-reader gates open',
      },
      null,
      2,
    ) + '\n',
  );
});
