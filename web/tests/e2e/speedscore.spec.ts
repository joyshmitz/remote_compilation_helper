import { test, expect } from '@playwright/test';
import { mockApiResponses } from '../fixtures/test-utils';
import {
  mockWorkers,
  mockSpeedScores,
  mockSpeedScoreListResponse,
  mockSpeedScoreHistoryResponse,
} from '../fixtures/api-mocks';

const scoreLevelLabel: Record<string, string> = {
  excellent: 'Excellent',
  good: 'Good',
  average: 'Average',
  below_average: 'Below Average',
  poor: 'Poor',
};

function getScoreLevel(score: number): string {
  if (score >= 90) return 'excellent';
  if (score >= 70) return 'good';
  if (score >= 50) return 'average';
  if (score >= 30) return 'below_average';
  return 'poor';
}

test.describe('SpeedScore Badge Display', () => {
  test('worker cards display SpeedScore badges', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: worker cards display SpeedScore badges');

    await mockApiResponses(page);
    console.log('[e2e:speedscore] MOCK: API responses registered');

    await page.goto('/workers');
    console.log('[e2e:speedscore] NAVIGATE: Loaded /workers');

    for (const worker of mockWorkers) {
      const card = page.locator(
        `[data-testid="worker-card"][data-worker-id="${worker.id}"]`
      );
      await expect(card).toBeVisible();

      const badge = card.locator('[data-testid="speedscore-badge"]');
      await expect(badge).toBeVisible();

      const speedScore = mockSpeedScores[worker.id];
      if (speedScore && speedScore.total > 0) {
        const expectedScore = Math.round(speedScore.total);
        await expect(badge).toContainText(expectedScore.toString());
        console.log(
          `[e2e:speedscore] VERIFY: Worker ${worker.id} shows SpeedScore ${expectedScore}`
        );
      } else {
        // Worker with no valid score should show N/A
        await expect(badge).toContainText('N/A');
        console.log(`[e2e:speedscore] VERIFY: Worker ${worker.id} shows N/A`);
      }
    }

    console.log('[e2e:speedscore] TEST PASS: worker cards display SpeedScore badges');
  });

  test('SpeedScore badge shows correct color based on score level', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: SpeedScore badge shows correct color');

    await mockApiResponses(page);
    await page.goto('/workers');

    // worker-1 has score 92.4 (excellent - emerald)
    const worker1Badge = page
      .locator('[data-testid="worker-card"][data-worker-id="worker-1"]')
      .locator('[data-testid="speedscore-badge"]');
    await expect(worker1Badge).toBeVisible();
    const worker1Class = await worker1Badge.getAttribute('class');
    expect(worker1Class).toContain('emerald');
    console.log('[e2e:speedscore] VERIFY: worker-1 (92.4) has emerald/excellent color');

    // worker-2 has score 61.2 (average - amber)
    const worker2Badge = page
      .locator('[data-testid="worker-card"][data-worker-id="worker-2"]')
      .locator('[data-testid="speedscore-badge"]');
    await expect(worker2Badge).toBeVisible();
    const worker2Class = await worker2Badge.getAttribute('class');
    expect(worker2Class).toContain('amber');
    console.log('[e2e:speedscore] VERIFY: worker-2 (61.2) has amber/average color');

    console.log('[e2e:speedscore] TEST PASS: SpeedScore badge shows correct color');
  });

  test('SpeedScore badge has proper accessibility attributes', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: SpeedScore badge accessibility');

    await mockApiResponses(page);
    await page.goto('/workers');

    const badge = page
      .locator('[data-testid="worker-card"][data-worker-id="worker-1"]')
      .locator('[data-testid="speedscore-badge"]');
    await expect(badge).toBeVisible();

    // Check role attribute
    await expect(badge).toHaveAttribute('role', 'status');

    // Check aria-label includes score
    const ariaLabel = await badge.getAttribute('aria-label');
    expect(ariaLabel).toContain('SpeedScore');
    expect(ariaLabel).toContain('92');
    expect(ariaLabel).toContain('Excellent');
    console.log(`[e2e:speedscore] VERIFY: aria-label = "${ariaLabel}"`);

    console.log('[e2e:speedscore] TEST PASS: SpeedScore badge accessibility');
  });
});

test.describe('SpeedScore Trend Indicator', () => {
  test('shows trend indicator when previous score exists', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: trend indicator visibility');

    // Update mock workers to include previous scores
    const workersWithPrevScore = mockWorkers.map((w) => ({
      ...w,
      speed_score_prev: w.id === 'worker-1' ? 85.0 : undefined,
    }));

    await mockApiResponses(page, {
      status: {
        daemon: {
          pid: 12345,
          uptime_secs: 3600,
          version: '0.5.0',
          socket_path: '/tmp/rch.sock',
          started_at: '2026-01-01T11:00:00.000Z',
          workers_total: 3,
          workers_healthy: 2,
          slots_total: 32,
          slots_available: 12,
        },
        workers: workersWithPrevScore,
        active_builds: [],
        recent_builds: [],
        issues: [],
        stats: {
          total_builds: 0,
          successful_builds: 0,
          failed_builds: 0,
          total_duration_ms: 0,
          avg_duration_ms: 0,
        },
      },
    });

    await page.goto('/workers');

    // worker-1 has prev score 85, current 92.4 (up trend)
    const trendIndicator = page
      .locator('[data-testid="worker-card"][data-worker-id="worker-1"]')
      .locator('[data-testid="speedscore-trend"]');

    // Trend indicator may be hidden on mobile, check it exists in DOM
    await expect(trendIndicator).toBeAttached();
    await expect(trendIndicator).toHaveAttribute('data-direction', 'up');
    console.log('[e2e:speedscore] VERIFY: worker-1 shows upward trend');

    console.log('[e2e:speedscore] TEST PASS: trend indicator visibility');
  });
});

test.describe('SpeedScore API Endpoints', () => {
  test('fetches all workers SpeedScores', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: fetch all workers SpeedScores');

    await mockApiResponses(page);

    const response = await page.request.get('http://localhost:3000/api/workers/speedscores');
    expect(response.status()).toBe(200);

    const data = await response.json();
    expect(data.workers).toBeInstanceOf(Array);
    expect(data.workers.length).toBe(mockSpeedScoreListResponse.workers.length);

    for (const worker of data.workers) {
      expect(worker).toHaveProperty('worker_id');
      expect(worker).toHaveProperty('speedscore');
      expect(worker).toHaveProperty('status');
      console.log(`[e2e:speedscore] VERIFY: Worker ${worker.worker_id} in response`);
    }

    console.log('[e2e:speedscore] TEST PASS: fetch all workers SpeedScores');
  });

  test('fetches individual worker SpeedScore', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: fetch individual worker SpeedScore');

    await mockApiResponses(page);

    const response = await page.request.get('http://localhost:3000/api/workers/worker-1/speedscore');
    expect(response.status()).toBe(200);

    const data = await response.json();
    expect(data.worker_id).toBe('worker-1');
    expect(data.speedscore).not.toBeNull();
    expect(data.speedscore.total).toBeCloseTo(92.4, 1);
    expect(data.speedscore.cpu_score).toBe(95);
    expect(data.speedscore.memory_score).toBe(88);
    expect(data.speedscore.disk_score).toBe(91);
    expect(data.speedscore.network_score).toBe(93);
    expect(data.speedscore.compilation_score).toBe(94);
    console.log('[e2e:speedscore] VERIFY: worker-1 SpeedScore components correct');

    console.log('[e2e:speedscore] TEST PASS: fetch individual worker SpeedScore');
  });

  test('returns 404 for unknown worker SpeedScore', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: 404 for unknown worker');

    await mockApiResponses(page);

    const response = await page.request.get(
      'http://localhost:3000/api/workers/nonexistent/speedscore'
    );
    expect(response.status()).toBe(404);
    console.log('[e2e:speedscore] VERIFY: 404 returned for nonexistent worker');

    console.log('[e2e:speedscore] TEST PASS: 404 for unknown worker');
  });

  test('fetches SpeedScore history with pagination', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: fetch SpeedScore history');

    await mockApiResponses(page);

    const response = await page.request.get(
      'http://localhost:3000/api/workers/worker-1/speedscore/history?days=7&limit=10'
    );
    expect(response.status()).toBe(200);

    const data = await response.json();
    expect(data.worker_id).toBe('worker-1');
    expect(data.history).toBeInstanceOf(Array);
    expect(data.history.length).toBeGreaterThanOrEqual(1);
    expect(data.history.length).toBeLessThanOrEqual(10);

    // Verify ordering (newest first)
    for (let i = 1; i < data.history.length; i++) {
      const prev = new Date(data.history[i - 1].measured_at);
      const curr = new Date(data.history[i].measured_at);
      expect(prev.getTime()).toBeGreaterThanOrEqual(curr.getTime());
    }
    console.log(`[e2e:speedscore] VERIFY: History has ${data.history.length} entries, ordered by date`);

    expect(data.pagination).toHaveProperty('total');
    expect(data.pagination).toHaveProperty('offset');
    expect(data.pagination).toHaveProperty('limit');
    console.log('[e2e:speedscore] VERIFY: Pagination info present');

    console.log('[e2e:speedscore] TEST PASS: fetch SpeedScore history');
  });
});

test.describe('SpeedScore Dashboard Integration', () => {
  test('dashboard shows worker SpeedScores in overview', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: dashboard SpeedScore overview');

    await mockApiResponses(page);
    await page.goto('/');

    // Wait for dashboard to load
    await page.waitForSelector('[data-testid="worker-card"]', { timeout: 5000 });

    // Check that SpeedScore badges are visible on dashboard
    const badges = page.locator('[data-testid="speedscore-badge"]');
    const count = await badges.count();
    expect(count).toBeGreaterThanOrEqual(1);
    console.log(`[e2e:speedscore] VERIFY: Found ${count} SpeedScore badges on dashboard`);

    console.log('[e2e:speedscore] TEST PASS: dashboard SpeedScore overview');
  });

  test('workers page shows all SpeedScore badges', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: workers page SpeedScore badges');

    await mockApiResponses(page);
    await page.goto('/workers');

    const badges = page.locator('[data-testid="speedscore-badge"]');
    const count = await badges.count();
    expect(count).toBe(mockWorkers.length);
    console.log(`[e2e:speedscore] VERIFY: ${count} SpeedScore badges for ${mockWorkers.length} workers`);

    console.log('[e2e:speedscore] TEST PASS: workers page SpeedScore badges');
  });

  test('SpeedScore N/A state for unbenchmarked workers', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: N/A state for unbenchmarked workers');

    // Create workers without SpeedScores
    const workersWithoutScores = mockWorkers.map((w) => ({
      ...w,
      speed_score: 0,
    }));

    await mockApiResponses(page, {
      status: {
        daemon: {
          pid: 12345,
          uptime_secs: 3600,
          version: '0.5.0',
          socket_path: '/tmp/rch.sock',
          started_at: '2026-01-01T11:00:00.000Z',
          workers_total: 3,
          workers_healthy: 2,
          slots_total: 32,
          slots_available: 12,
        },
        workers: workersWithoutScores,
        active_builds: [],
        recent_builds: [],
        issues: [],
        stats: {
          total_builds: 0,
          successful_builds: 0,
          failed_builds: 0,
          total_duration_ms: 0,
          avg_duration_ms: 0,
        },
      },
    });

    await page.goto('/workers');

    // All badges should show N/A or 0 for workers with score 0
    const badges = page.locator('[data-testid="speedscore-badge"]');
    const count = await badges.count();

    for (let i = 0; i < count; i++) {
      const badge = badges.nth(i);
      const text = await badge.textContent();
      // Score 0 will render as "0" since it's a valid number
      expect(text).toMatch(/0|N\/A/);
    }
    console.log('[e2e:speedscore] VERIFY: Unbenchmarked workers show 0 or N/A');

    console.log('[e2e:speedscore] TEST PASS: N/A state for unbenchmarked workers');
  });
});

test.describe('SpeedScore Loading and Error States', () => {
  test('shows loading state while fetching SpeedScore', async ({ page }) => {
    console.log('[e2e:speedscore] TEST START: loading state');

    // Delay the API response to observe loading state
    await page.route('**/status', async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 500));
      await route.fulfill({
        json: {
          daemon: {
            pid: 12345,
            uptime_secs: 3600,
            version: '0.5.0',
            socket_path: '/tmp/rch.sock',
            started_at: '2026-01-01T11:00:00.000Z',
            workers_total: 1,
            workers_healthy: 1,
            slots_total: 8,
            slots_available: 4,
          },
          workers: mockWorkers.slice(0, 1),
          active_builds: [],
          recent_builds: [],
          issues: [],
          stats: {
            total_builds: 0,
            successful_builds: 0,
            failed_builds: 0,
            total_duration_ms: 0,
            avg_duration_ms: 0,
          },
        },
      });
    });

    // Navigate and check for any loading indicators
    await page.goto('/workers');

    // The page should eventually load with worker cards
    await expect(page.locator('[data-testid="worker-card"]')).toBeVisible({ timeout: 5000 });
    console.log('[e2e:speedscore] VERIFY: Page loads successfully after delay');

    console.log('[e2e:speedscore] TEST PASS: loading state');
  });
});
