import { describe, it, expect } from 'vitest';
import {
  findBestScores,
  findBestComponentScores,
  calculatePerformanceRatio,
  getPerformanceTier,
  sortWorkersByScore,
  getTopWorkers,
  filterWorkersByStatus,
  getHealthyWorkers,
  generateShareableParams,
  parseWorkerIdsFromParams,
} from './comparison-utils';
import type { WorkerStatusInfo, SpeedScoreView } from './types';

describe('comparison-utils', () => {
  const mockWorkers: WorkerStatusInfo[] = [
    {
      id: 'worker-1',
      host: 'host1.example.com',
      user: 'ubuntu',
      status: 'healthy',
      circuit_state: 'closed',
      used_slots: 2,
      total_slots: 8,
      speed_score: 85,
      last_error: null,
    },
    {
      id: 'worker-2',
      host: 'host2.example.com',
      user: 'ubuntu',
      status: 'healthy',
      circuit_state: 'closed',
      used_slots: 4,
      total_slots: 16,
      speed_score: 92,
      last_error: null,
    },
    {
      id: 'worker-3',
      host: 'host3.example.com',
      user: 'ubuntu',
      status: 'degraded',
      circuit_state: 'half_open',
      used_slots: 0,
      total_slots: 4,
      speed_score: 65,
      last_error: 'Connection timeout',
    },
  ];

  const mockSpeedScores: SpeedScoreView[] = [
    {
      total: 85,
      cpu_score: 90,
      memory_score: 80,
      disk_score: 85,
      network_score: 82,
      compilation_score: 88,
      measured_at: '2026-01-15T10:00:00Z',
      version: 1,
    },
    {
      total: 92,
      cpu_score: 88,
      memory_score: 95,
      disk_score: 90,
      network_score: 94,
      compilation_score: 91,
      measured_at: '2026-01-15T10:00:00Z',
      version: 1,
    },
  ];

  describe('findBestScores', () => {
    it('finds the highest speed score and total slots', () => {
      const result = findBestScores(mockWorkers);
      expect(result.speedScore).toBe(92);
      expect(result.totalSlots).toBe(16);
    });

    it('returns zeros for empty array', () => {
      const result = findBestScores([]);
      expect(result.speedScore).toBe(0);
      expect(result.totalSlots).toBe(0);
    });
  });

  describe('findBestComponentScores', () => {
    it('finds the highest score for each component', () => {
      const result = findBestComponentScores(mockSpeedScores);
      expect(result.total).toBe(92);
      expect(result.cpu_score).toBe(90);
      expect(result.memory_score).toBe(95);
      expect(result.disk_score).toBe(90);
      expect(result.network_score).toBe(94);
      expect(result.compilation_score).toBe(91);
    });

    it('returns zeros for empty array', () => {
      const result = findBestComponentScores([]);
      expect(result.total).toBe(0);
      expect(result.cpu_score).toBe(0);
    });
  });

  describe('calculatePerformanceRatio', () => {
    it('calculates ratio correctly', () => {
      expect(calculatePerformanceRatio(85, 100)).toBe(0.85);
      expect(calculatePerformanceRatio(100, 100)).toBe(1);
      expect(calculatePerformanceRatio(50, 100)).toBe(0.5);
    });

    it('returns 0 when best is 0', () => {
      expect(calculatePerformanceRatio(50, 0)).toBe(0);
    });
  });

  describe('getPerformanceTier', () => {
    it('returns "best" for ratio >= 1', () => {
      expect(getPerformanceTier(1)).toBe('best');
      expect(getPerformanceTier(1.05)).toBe('best');
    });

    it('returns "excellent" for ratio >= 0.95', () => {
      expect(getPerformanceTier(0.95)).toBe('excellent');
      expect(getPerformanceTier(0.98)).toBe('excellent');
    });

    it('returns "good" for ratio >= 0.85', () => {
      expect(getPerformanceTier(0.85)).toBe('good');
      expect(getPerformanceTier(0.90)).toBe('good');
    });

    it('returns "average" for ratio >= 0.70', () => {
      expect(getPerformanceTier(0.70)).toBe('average');
      expect(getPerformanceTier(0.80)).toBe('average');
    });

    it('returns "below_average" for ratio < 0.70', () => {
      expect(getPerformanceTier(0.69)).toBe('below_average');
      expect(getPerformanceTier(0.50)).toBe('below_average');
    });
  });

  describe('sortWorkersByScore', () => {
    it('sorts workers by speed score descending', () => {
      const result = sortWorkersByScore(mockWorkers);
      expect(result[0].id).toBe('worker-2'); // 92
      expect(result[1].id).toBe('worker-1'); // 85
      expect(result[2].id).toBe('worker-3'); // 65
    });

    it('does not mutate original array', () => {
      const original = [...mockWorkers];
      sortWorkersByScore(mockWorkers);
      expect(mockWorkers[0].id).toBe(original[0].id);
    });
  });

  describe('getTopWorkers', () => {
    it('returns top N workers by score', () => {
      const result = getTopWorkers(mockWorkers, 2);
      expect(result).toHaveLength(2);
      expect(result[0].id).toBe('worker-2');
      expect(result[1].id).toBe('worker-1');
    });

    it('returns all workers if N > length', () => {
      const result = getTopWorkers(mockWorkers, 10);
      expect(result).toHaveLength(3);
    });
  });

  describe('filterWorkersByStatus', () => {
    it('filters workers by single status', () => {
      const result = filterWorkersByStatus(mockWorkers, ['healthy']);
      expect(result).toHaveLength(2);
      expect(result.every((w) => w.status === 'healthy')).toBe(true);
    });

    it('filters workers by multiple statuses', () => {
      const result = filterWorkersByStatus(mockWorkers, ['healthy', 'degraded']);
      expect(result).toHaveLength(3);
    });

    it('returns empty array when no match', () => {
      const result = filterWorkersByStatus(mockWorkers, ['unreachable']);
      expect(result).toHaveLength(0);
    });
  });

  describe('getHealthyWorkers', () => {
    it('returns only healthy workers', () => {
      const result = getHealthyWorkers(mockWorkers);
      expect(result).toHaveLength(2);
      expect(result.every((w) => w.status === 'healthy')).toBe(true);
    });
  });

  describe('generateShareableParams', () => {
    it('generates URL params from worker IDs', () => {
      expect(generateShareableParams(['w1', 'w2', 'w3'])).toBe('workers=w1,w2,w3');
    });

    it('returns empty string for empty array', () => {
      expect(generateShareableParams([])).toBe('');
    });
  });

  describe('parseWorkerIdsFromParams', () => {
    it('parses comma-separated worker IDs', () => {
      const result = parseWorkerIdsFromParams('w1,w2,w3');
      expect(result).toEqual(['w1', 'w2', 'w3']);
    });

    it('handles empty or trailing commas', () => {
      const result = parseWorkerIdsFromParams('w1,,w2,');
      expect(result).toEqual(['w1', 'w2']);
    });

    it('returns empty array for null', () => {
      expect(parseWorkerIdsFromParams(null)).toEqual([]);
    });

    it('returns empty array for empty string', () => {
      expect(parseWorkerIdsFromParams('')).toEqual([]);
    });
  });
});
