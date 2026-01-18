import type { SpeedScoreView } from './types';

/**
 * Chart data point with parsed date for Recharts
 */
export interface ChartDataPoint {
  measured_at: Date;
  timestamp: number;
  total: number;
  cpu_score: number;
  memory_score: number;
  disk_score: number;
  network_score: number;
  compilation_score: number;
}

/**
 * Date range preset options
 */
export interface DateRangePreset {
  label: string;
  days: number | null;
}

export const DATE_RANGE_PRESETS: DateRangePreset[] = [
  { label: '24h', days: 1 },
  { label: '7d', days: 7 },
  { label: '30d', days: 30 },
  { label: '90d', days: 90 },
  { label: 'All', days: null },
];

/**
 * Transform SpeedScoreView array to chart data format
 */
export function transformToChartData(history: SpeedScoreView[]): ChartDataPoint[] {
  return history
    .map((entry) => {
      const date = new Date(entry.measured_at);
      return {
        measured_at: date,
        timestamp: date.getTime(),
        total: entry.total,
        cpu_score: entry.cpu_score,
        memory_score: entry.memory_score,
        disk_score: entry.disk_score,
        network_score: entry.network_score,
        compilation_score: entry.compilation_score,
      };
    })
    .sort((a, b) => a.timestamp - b.timestamp);
}

/**
 * Filter data by date range
 */
export function filterByDateRange(
  data: ChartDataPoint[],
  startDate: Date | null,
  endDate: Date | null
): ChartDataPoint[] {
  if (!startDate && !endDate) return data;

  const startTs = startDate?.getTime() ?? -Infinity;
  const endTs = endDate?.getTime() ?? Infinity;

  return data.filter((point) => point.timestamp >= startTs && point.timestamp <= endTs);
}

/**
 * Get date range from preset
 */
export function getDateRangeFromPreset(preset: DateRangePreset): [Date | null, Date | null] {
  if (preset.days === null) {
    return [null, null];
  }
  const end = new Date();
  const start = new Date();
  start.setDate(start.getDate() - preset.days);
  return [start, end];
}

/**
 * Downsample data points for performance
 * Uses LTTB (Largest Triangle Three Buckets) algorithm
 */
export function downsampleData(data: ChartDataPoint[], targetPoints: number): ChartDataPoint[] {
  if (data.length <= targetPoints) return data;
  if (targetPoints <= 2) return [data[0], data[data.length - 1]];

  const bucketSize = (data.length - 2) / (targetPoints - 2);
  const result: ChartDataPoint[] = [data[0]];

  for (let i = 0; i < targetPoints - 2; i++) {
    const bucketStart = Math.floor((i + 0) * bucketSize) + 1;
    const bucketEnd = Math.floor((i + 1) * bucketSize) + 1;
    const nextBucketStart = Math.floor((i + 1) * bucketSize) + 1;
    const nextBucketEnd = Math.min(Math.floor((i + 2) * bucketSize) + 1, data.length - 1);

    // Calculate average point in next bucket
    let avgX = 0;
    let avgY = 0;
    for (let j = nextBucketStart; j < nextBucketEnd; j++) {
      avgX += data[j].timestamp;
      avgY += data[j].total;
    }
    const nextBucketSize = nextBucketEnd - nextBucketStart;
    avgX /= nextBucketSize;
    avgY /= nextBucketSize;

    // Find point in current bucket with largest triangle area
    const a = result[result.length - 1];
    let maxArea = -1;
    let maxAreaPoint = data[bucketStart];

    for (let j = bucketStart; j < bucketEnd; j++) {
      const point = data[j];
      const area = Math.abs(
        (a.timestamp - avgX) * (point.total - a.total) -
          (a.timestamp - point.timestamp) * (avgY - a.total)
      );
      if (area > maxArea) {
        maxArea = area;
        maxAreaPoint = point;
      }
    }
    result.push(maxAreaPoint);
  }

  result.push(data[data.length - 1]);
  return result;
}

/**
 * Score component configuration for chart styling
 */
export interface ScoreComponentConfig {
  key: keyof Omit<ChartDataPoint, 'measured_at' | 'timestamp'>;
  label: string;
  color: string;
}

export const SCORE_COMPONENTS: ScoreComponentConfig[] = [
  { key: 'cpu_score', label: 'CPU', color: '#22c55e' },
  { key: 'memory_score', label: 'Memory', color: '#eab308' },
  { key: 'disk_score', label: 'Disk', color: '#f97316' },
  { key: 'network_score', label: 'Network', color: '#8b5cf6' },
  { key: 'compilation_score', label: 'Compilation', color: '#ec4899' },
];

export const TOTAL_LINE_COLOR = '#3b82f6';
