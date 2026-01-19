'use client';

import { AlertCircle, TrendingUp, TrendingDown, Minus } from 'lucide-react';
import { cn } from '@/lib/utils';
import { Tooltip } from '@/components/ui/tooltip';

// ============================================================================
// Types
// ============================================================================

type ScoreLevel = 'excellent' | 'good' | 'average' | 'below_average' | 'poor';
type TrendDirection = 'up' | 'down' | 'stable';
type TrendMagnitude = 'small' | 'medium' | 'large';

interface Trend {
  direction: TrendDirection;
  magnitude: TrendMagnitude;
  delta: number;
}

/**
 * Component breakdown for tooltip display
 */
export interface SpeedScoreBreakdown {
  cpu_score: number;
  memory_score: number;
  disk_score: number;
  network_score: number;
  compilation_score: number;
  measured_at?: string;
}

export interface SpeedScoreBadgeProps {
  /** The total SpeedScore (0-100) */
  score: number | null | undefined;
  /** Previous score for trend calculation */
  previousScore?: number | null;
  /** Component breakdown for rich tooltip */
  breakdown?: SpeedScoreBreakdown | null;
  /** Badge size variant */
  size?: 'sm' | 'md' | 'lg';
  /** Whether to show trend indicator */
  showTrend?: boolean;
  /** Loading state */
  isLoading?: boolean;
  /** Error state */
  error?: Error | null;
  /** Callback for retry on error */
  onRetry?: () => void;
}

// ============================================================================
// Constants
// ============================================================================

const SCORE_LEVELS: Record<ScoreLevel, { min: number; max: number; label: string; className: string }> = {
  excellent: {
    min: 90,
    max: 100,
    label: 'Excellent',
    className: 'border-emerald-600/40 bg-emerald-500/15 text-emerald-800 dark:text-emerald-200 dark:bg-emerald-500/20',
  },
  good: {
    min: 70,
    max: 89,
    label: 'Good',
    className: 'border-sky-600/40 bg-sky-500/15 text-sky-800 dark:text-sky-200 dark:bg-sky-500/20',
  },
  average: {
    min: 50,
    max: 69,
    label: 'Average',
    className: 'border-amber-600/40 bg-amber-500/15 text-amber-900 dark:text-amber-200 dark:bg-amber-500/20',
  },
  below_average: {
    min: 30,
    max: 49,
    label: 'Below Average',
    className: 'border-orange-600/40 bg-orange-500/15 text-orange-900 dark:text-orange-200 dark:bg-orange-500/20',
  },
  poor: {
    min: 0,
    max: 29,
    label: 'Poor',
    className: 'border-red-600/40 bg-red-500/15 text-red-800 dark:text-red-200 dark:bg-red-500/20',
  },
};

const BADGE_SIZE_CLASSES = {
  sm: 'px-2 py-0.5 text-[11px]',
  md: 'px-2.5 py-1 text-xs',
  lg: 'px-3 py-1.5 text-sm',
};

// ============================================================================
// Utility Functions
// ============================================================================

function clampScore(score: number): number {
  return Math.min(100, Math.max(0, score));
}

export function getScoreLevel(score: number): ScoreLevel {
  if (score >= 90) return 'excellent';
  if (score >= 70) return 'good';
  if (score >= 50) return 'average';
  if (score >= 30) return 'below_average';
  return 'poor';
}

export function getScoreLabel(score: number): string {
  return SCORE_LEVELS[getScoreLevel(score)].label;
}

export function getScoreColorClass(score: number): string {
  return SCORE_LEVELS[getScoreLevel(score)].className;
}

export function calculateTrend(current: number, previous: number): Trend {
  const delta = current - previous;
  const absDelta = Math.abs(delta);
  const baseline = Math.max(Math.abs(previous), 1);
  const percentChange = (absDelta / baseline) * 100;

  let direction: TrendDirection = 'stable';
  if (absDelta >= 1) {
    direction = delta > 0 ? 'up' : 'down';
  }

  let magnitude: TrendMagnitude = 'small';
  if (percentChange >= 15) {
    magnitude = 'large';
  } else if (percentChange >= 5) {
    magnitude = 'medium';
  }

  return { direction, magnitude, delta };
}

function formatRelativeTime(dateStr: string | undefined): string {
  if (!dateStr) return 'Unknown';
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

// ============================================================================
// TrendIndicator Component
// ============================================================================

interface TrendIndicatorProps {
  trend: Trend;
}

export function TrendIndicator({ trend }: TrendIndicatorProps) {
  const Icon = trend.direction === 'up' ? TrendingUp : trend.direction === 'down' ? TrendingDown : Minus;
  const tone =
    trend.direction === 'up'
      ? 'text-emerald-600 dark:text-emerald-300'
      : trend.direction === 'down'
        ? 'text-red-600 dark:text-red-300'
        : 'text-muted-foreground';

  return (
    <span
      className={cn('hidden sm:inline-flex items-center', tone)}
      data-testid="speedscore-trend"
      data-direction={trend.direction}
      data-magnitude={trend.magnitude}
      aria-label={`Trend: ${trend.direction}, ${Math.abs(trend.delta).toFixed(1)} points`}
      title={`${trend.delta >= 0 ? '+' : ''}${trend.delta.toFixed(1)} since last benchmark`}
    >
      <Icon className="h-3.5 w-3.5" aria-hidden="true" />
    </span>
  );
}

// ============================================================================
// SpeedScoreTooltipContent Component
// ============================================================================

interface SpeedScoreTooltipContentProps {
  score: number;
  breakdown?: SpeedScoreBreakdown | null;
}

function SpeedScoreTooltipContent({ score, breakdown }: SpeedScoreTooltipContentProps) {
  const level = getScoreLevel(score);
  const levelConfig = SCORE_LEVELS[level];

  if (!breakdown) {
    return `SpeedScore ${Math.round(score)} (${levelConfig.label})`;
  }

  const rows = [
    { label: 'CPU', value: breakdown.cpu_score },
    { label: 'Memory', value: breakdown.memory_score },
    { label: 'Disk', value: breakdown.disk_score },
    { label: 'Network', value: breakdown.network_score },
    { label: 'Compilation', value: breakdown.compilation_score },
  ];

  const content = [
    `SpeedScore: ${Math.round(score)}/100 (${levelConfig.label})`,
    '',
    ...rows.map(r => `${r.label}: ${r.value.toFixed(0)}`),
    '',
    `Last: ${formatRelativeTime(breakdown.measured_at)}`,
  ];

  return content.join('\n');
}

// ============================================================================
// SpeedScoreBadge Component
// ============================================================================

export function SpeedScoreBadge({
  score,
  previousScore,
  breakdown,
  size = 'sm',
  showTrend = true,
  isLoading = false,
  error = null,
  onRetry,
}: SpeedScoreBadgeProps) {
  const baseClass = cn(
    'inline-flex items-center gap-1 rounded-full border font-semibold leading-none',
    BADGE_SIZE_CLASSES[size]
  );

  // Loading state
  if (isLoading) {
    return (
      <span
        className={cn(baseClass, 'bg-muted/40 text-muted-foreground animate-pulse')}
        aria-busy="true"
        data-testid="speedscore-badge"
      >
        <span className="inline-block h-3 w-6 rounded bg-muted" />
      </span>
    );
  }

  // Error state
  if (error) {
    return (
      <button
        type="button"
        onClick={onRetry}
        className={cn(baseClass, 'bg-error/10 text-error border-error/30')}
        aria-label="SpeedScore failed to load. Click to retry."
        data-testid="speedscore-badge"
      >
        <AlertCircle className="h-3.5 w-3.5" aria-hidden="true" />
        <span>Error</span>
      </button>
    );
  }

  // Validate numeric score
  const numericScore = typeof score === 'number' && Number.isFinite(score) ? score : null;

  // Not benchmarked state
  if (numericScore === null) {
    return (
      <span
        className={cn(baseClass, 'bg-muted/50 text-muted-foreground border-border')}
        aria-label="Not benchmarked"
        data-testid="speedscore-badge"
      >
        N/A
      </span>
    );
  }

  // Normal state with score
  const clamped = clampScore(numericScore);
  const level = getScoreLevel(clamped);
  const levelConfig = SCORE_LEVELS[level];
  const trend =
    showTrend && typeof previousScore === 'number' && Number.isFinite(previousScore)
      ? calculateTrend(clamped, previousScore)
      : null;

  const tooltipContent = SpeedScoreTooltipContent({ score: clamped, breakdown });

  const badge = (
    <span
      className={cn(baseClass, levelConfig.className)}
      role="status"
      aria-label={`SpeedScore: ${Math.round(clamped)} out of 100, ${levelConfig.label}`}
      data-testid="speedscore-badge"
    >
      <span className="tabular-nums">{Math.round(clamped)}</span>
      {trend ? <TrendIndicator trend={trend} /> : null}
    </span>
  );

  // Wrap in tooltip if we have content
  if (breakdown) {
    return <Tooltip content={tooltipContent}>{badge}</Tooltip>;
  }

  // Add title attribute for simple tooltip when no breakdown
  return (
    <span title={tooltipContent}>
      {badge}
    </span>
  );
}

export default SpeedScoreBadge;
