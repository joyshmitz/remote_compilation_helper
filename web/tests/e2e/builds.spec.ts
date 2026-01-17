import { test, expect } from '@playwright/test';
import { mockApiResponses } from '../fixtures/test-utils';
import { mockStatusResponse, mockRecentBuilds, mockActiveBuilds } from '../fixtures/api-mocks';

const statusLabels: Record<number, string> = {
  0: 'Success',
};

test('builds table loads with recent builds', async ({ page }) => {
  console.log('[e2e:builds] TEST START: builds table loads with recent builds');

  await mockApiResponses(page);
  console.log('[e2e:builds] MOCK: status/health/ready/budget/metrics');

  await page.goto('/builds');
  console.log('[e2e:builds] NAVIGATE: Loaded /builds');

  const table = page.locator('[data-testid="builds-table"]');
  await expect(table).toBeVisible();
  console.log('[e2e:builds] VERIFY: builds table visible');

  await expect(page.getByRole('columnheader', { name: 'Project' })).toBeVisible();
  await expect(page.getByRole('columnheader', { name: 'Worker' })).toBeVisible();
  await expect(page.getByRole('columnheader', { name: 'Duration' })).toBeVisible();
  await expect(page.getByRole('columnheader', { name: 'Status' })).toBeVisible();
  await expect(page.getByRole('columnheader', { name: 'Time' })).toBeVisible();
  console.log('[e2e:builds] VERIFY: table headers rendered');

  const rows = page.locator('[data-testid="build-row"]');
  await expect(rows).toHaveCount(mockRecentBuilds.length);
  console.log('[e2e:builds] VERIFY: row count matches mock data');

  console.log('[e2e:builds] TEST PASS: builds table loads with recent builds');
});

test('active builds section renders when active builds exist', async ({ page }) => {
  console.log('[e2e:builds] TEST START: active builds section renders');

  await mockApiResponses(page);
  console.log('[e2e:builds] MOCK: status/health/ready/budget/metrics');

  await page.goto('/builds');
  console.log('[e2e:builds] NAVIGATE: Loaded /builds');

  const activeBuilds = page.locator('[data-testid="active-build"]');
  await expect(activeBuilds).toHaveCount(mockActiveBuilds.length);
  console.log('[e2e:builds] VERIFY: active build rows rendered');

  for (const build of mockActiveBuilds) {
    const row = page.locator(`[data-build-id="${build.id}"]`);
    await expect(row).toContainText(build.command);
    await expect(row).toContainText(build.worker_id);
    console.log(`[e2e:builds] VERIFY: active build ${build.id} content visible`);
  }

  console.log('[e2e:builds] TEST PASS: active builds section renders');
});

test('build rows show success and failure status', async ({ page }) => {
  console.log('[e2e:builds] TEST START: build rows show success and failure status');

  await mockApiResponses(page);
  console.log('[e2e:builds] MOCK: status/health/ready/budget/metrics');

  await page.goto('/builds');
  console.log('[e2e:builds] NAVIGATE: Loaded /builds');

  for (const build of mockRecentBuilds) {
    const row = page.locator(`[data-build-id="${build.id}"]`);
    const expected = build.exit_code === 0 ? statusLabels[0] : `Exit ${build.exit_code}`;
    console.log(`[e2e:builds] VERIFY: build ${build.id} status ${expected}`);
    await expect(row).toContainText(expected);
  }

  console.log('[e2e:builds] TEST PASS: build rows show success and failure status');
});

test('builds page shows empty state when no recent builds', async ({ page }) => {
  console.log('[e2e:builds] TEST START: builds page shows empty state when no recent builds');

  const emptyStatus = { ...mockStatusResponse, recent_builds: [], active_builds: [] };
  await mockApiResponses(page, { status: emptyStatus });
  console.log('[e2e:builds] MOCK: status with zero recent builds');

  await page.goto('/builds');
  console.log('[e2e:builds] NAVIGATE: Loaded /builds');

  await expect(page.getByText('No builds recorded yet')).toBeVisible();
  console.log('[e2e:builds] VERIFY: empty state text visible');

  console.log('[e2e:builds] TEST PASS: empty state when no recent builds');
});

test('builds page shows error state on API failure', async ({ page }) => {
  console.log('[e2e:builds] TEST START: builds page shows error state on API failure');

  await page.route('**/status', async (route) => {
    console.log('[mock] Intercepting /status (500)');
    await route.fulfill({ status: 500, body: 'boom', contentType: 'text/plain' });
  });

  await page.goto('/builds');
  console.log('[e2e:builds] NAVIGATE: Loaded /builds');

  const errorState = page.locator('[data-testid="error-state"]');
  await expect(errorState).toBeVisible();
  console.log('[e2e:builds] VERIFY: error state visible');

  await expect(page.getByRole('button', { name: /try again/i })).toBeVisible();
  console.log('[e2e:builds] VERIFY: retry button visible');

  console.log('[e2e:builds] TEST PASS: error state on API failure');
});
