import { describe, it, expect } from 'vitest';
import {
  transformToChartData,
  filterByDateRange,
  downsampleData,
  getDateRangeFromPreset,
  DATE_RANGE_PRESETS,
  SCORE_COMPONENTS,
} from './chart-utils';
import type { SpeedScoreView } from './types';

describe('chart-utils', () => {
  const mockHistory: SpeedScoreView[] = [
    {
      total: 85,
      cpu_score: 90,
      memory_score: 80,
      disk_score: 85,
      network_score: 82,
      compilation_score: 88,
      measured_at: '2026-01-10T10:00:00Z',
      version: 1,
    },
    {
      total: 87,
      cpu_score: 91,
      memory_score: 82,
      disk_score: 86,
      network_score: 84,
      compilation_score: 90,
      measured_at: '2026-01-15T10:00:00Z',
      version: 1,
    },
    {
      total: 83,
      cpu_score: 88,
      memory_score: 78,
      disk_score: 83,
      network_score: 80,
      compilation_score: 86,
      measured_at: '2026-01-12T10:00:00Z',
      version: 1,
    },
  ];

  describe('transformToChartData', () => {
    it('transforms SpeedScoreView array to ChartDataPoint array', () => {
      const result = transformToChartData(mockHistory);

      expect(result).toHaveLength(3);
      expect(result[0].total).toBe(85);
      expect(result[0].cpu_score).toBe(90);
      expect(result[0].measured_at).toBeInstanceOf(Date);
      expect(result[0].timestamp).toBeTypeOf('number');
    });

    it('sorts data by timestamp ascending', () => {
      const result = transformToChartData(mockHistory);

      // Should be sorted: Jan 10, Jan 12, Jan 15
      expect(result[0].total).toBe(85); // Jan 10
      expect(result[1].total).toBe(83); // Jan 12
      expect(result[2].total).toBe(87); // Jan 15
    });

    it('handles empty array', () => {
      const result = transformToChartData([]);
      expect(result).toEqual([]);
    });
  });

  describe('filterByDateRange', () => {
    it('filters data within date range', () => {
      const data = transformToChartData(mockHistory);
      const start = new Date('2026-01-11T00:00:00Z');
      const end = new Date('2026-01-14T00:00:00Z');

      const result = filterByDateRange(data, start, end);

      expect(result).toHaveLength(1);
      expect(result[0].total).toBe(83); // Jan 12
    });

    it('returns all data when no range specified', () => {
      const data = transformToChartData(mockHistory);
      const result = filterByDateRange(data, null, null);

      expect(result).toHaveLength(3);
    });

    it('handles open-ended start date', () => {
      const data = transformToChartData(mockHistory);
      const end = new Date('2026-01-11T00:00:00Z');

      const result = filterByDateRange(data, null, end);

      expect(result).toHaveLength(1);
      expect(result[0].total).toBe(85); // Jan 10
    });

    it('handles open-ended end date', () => {
      const data = transformToChartData(mockHistory);
      const start = new Date('2026-01-14T00:00:00Z');

      const result = filterByDateRange(data, start, null);

      expect(result).toHaveLength(1);
      expect(result[0].total).toBe(87); // Jan 15
    });
  });

  describe('downsampleData', () => {
    it('returns original data when below target points', () => {
      const data = transformToChartData(mockHistory);
      const result = downsampleData(data, 10);

      expect(result).toHaveLength(3);
    });

    it('downsamples data when above target points', () => {
      // Create larger dataset
      const largeHistory: SpeedScoreView[] = Array.from({ length: 100 }, (_, i) => ({
        total: 80 + (i % 20),
        cpu_score: 85,
        memory_score: 80,
        disk_score: 82,
        network_score: 78,
        compilation_score: 84,
        measured_at: new Date(Date.now() - i * 86400000).toISOString(),
        version: 1,
      }));

      const data = transformToChartData(largeHistory);
      const result = downsampleData(data, 20);

      expect(result.length).toBeLessThanOrEqual(20);
      expect(result.length).toBeGreaterThan(0);
      // First and last points should be preserved
      expect(result[0].timestamp).toBe(data[0].timestamp);
      expect(result[result.length - 1].timestamp).toBe(data[data.length - 1].timestamp);
    });

    it('handles edge case of 2 target points', () => {
      const data = transformToChartData(mockHistory);
      const result = downsampleData(data, 2);

      expect(result).toHaveLength(2);
      expect(result[0].timestamp).toBe(data[0].timestamp);
      expect(result[1].timestamp).toBe(data[data.length - 1].timestamp);
    });
  });

  describe('getDateRangeFromPreset', () => {
    it('returns null range for "All" preset', () => {
      const preset = DATE_RANGE_PRESETS.find((p) => p.label === 'All')!;
      const [start, end] = getDateRangeFromPreset(preset);

      expect(start).toBeNull();
      expect(end).toBeNull();
    });

    it('returns correct range for day-based presets', () => {
      const preset = DATE_RANGE_PRESETS.find((p) => p.label === '7d')!;
      const [start, end] = getDateRangeFromPreset(preset);

      expect(start).toBeInstanceOf(Date);
      expect(end).toBeInstanceOf(Date);

      const daysDiff = (end!.getTime() - start!.getTime()) / (1000 * 60 * 60 * 24);
      expect(daysDiff).toBeCloseTo(7, 0);
    });
  });

  describe('DATE_RANGE_PRESETS', () => {
    it('has expected presets', () => {
      const labels = DATE_RANGE_PRESETS.map((p) => p.label);
      expect(labels).toContain('24h');
      expect(labels).toContain('7d');
      expect(labels).toContain('30d');
      expect(labels).toContain('90d');
      expect(labels).toContain('All');
    });
  });

  describe('SCORE_COMPONENTS', () => {
    it('has all score component configurations', () => {
      const keys = SCORE_COMPONENTS.map((c) => c.key);
      expect(keys).toContain('cpu_score');
      expect(keys).toContain('memory_score');
      expect(keys).toContain('disk_score');
      expect(keys).toContain('network_score');
      expect(keys).toContain('compilation_score');
    });

    it('has unique colors for each component', () => {
      const colors = SCORE_COMPONENTS.map((c) => c.color);
      const uniqueColors = new Set(colors);
      expect(uniqueColors.size).toBe(SCORE_COMPONENTS.length);
    });
  });
});
