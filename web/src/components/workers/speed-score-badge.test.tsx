import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import {
  SpeedScoreBadge,
  TrendIndicator,
  getScoreLevel,
  getScoreLabel,
  getScoreColorClass,
  calculateTrend,
  type SpeedScoreBreakdown,
} from './speed-score-badge';

// Mock the Tooltip component to avoid portal/popover complexity in tests
vi.mock('@/components/ui/tooltip', () => ({
  Tooltip: ({ children, content }: { children: React.ReactNode; content: string }) => (
    <div data-testid="tooltip-wrapper" data-content={content}>
      {children}
    </div>
  ),
}));

describe('getScoreLevel', () => {
  it('returns excellent for scores >= 90', () => {
    expect(getScoreLevel(90)).toBe('excellent');
    expect(getScoreLevel(100)).toBe('excellent');
    expect(getScoreLevel(95)).toBe('excellent');
  });

  it('returns good for scores 70-89', () => {
    expect(getScoreLevel(70)).toBe('good');
    expect(getScoreLevel(89)).toBe('good');
    expect(getScoreLevel(80)).toBe('good');
  });

  it('returns average for scores 50-69', () => {
    expect(getScoreLevel(50)).toBe('average');
    expect(getScoreLevel(69)).toBe('average');
    expect(getScoreLevel(60)).toBe('average');
  });

  it('returns below_average for scores 30-49', () => {
    expect(getScoreLevel(30)).toBe('below_average');
    expect(getScoreLevel(49)).toBe('below_average');
    expect(getScoreLevel(40)).toBe('below_average');
  });

  it('returns poor for scores < 30', () => {
    expect(getScoreLevel(0)).toBe('poor');
    expect(getScoreLevel(29)).toBe('poor');
    expect(getScoreLevel(15)).toBe('poor');
  });
});

describe('getScoreLabel', () => {
  it('returns correct labels for each level', () => {
    expect(getScoreLabel(95)).toBe('Excellent');
    expect(getScoreLabel(80)).toBe('Good');
    expect(getScoreLabel(60)).toBe('Average');
    expect(getScoreLabel(40)).toBe('Below Average');
    expect(getScoreLabel(20)).toBe('Poor');
  });
});

describe('getScoreColorClass', () => {
  it('returns emerald class for excellent scores', () => {
    expect(getScoreColorClass(95)).toContain('emerald');
  });

  it('returns sky class for good scores', () => {
    expect(getScoreColorClass(80)).toContain('sky');
  });

  it('returns amber class for average scores', () => {
    expect(getScoreColorClass(60)).toContain('amber');
  });

  it('returns orange class for below average scores', () => {
    expect(getScoreColorClass(40)).toContain('orange');
  });

  it('returns red class for poor scores', () => {
    expect(getScoreColorClass(20)).toContain('red');
  });
});

describe('calculateTrend', () => {
  it('returns up direction when current > previous', () => {
    const trend = calculateTrend(80, 70);
    expect(trend.direction).toBe('up');
    expect(trend.delta).toBe(10);
  });

  it('returns down direction when current < previous', () => {
    const trend = calculateTrend(70, 80);
    expect(trend.direction).toBe('down');
    expect(trend.delta).toBe(-10);
  });

  it('returns stable direction when delta < 1', () => {
    const trend = calculateTrend(80, 80.5);
    expect(trend.direction).toBe('stable');
  });

  it('returns small magnitude for < 5% change', () => {
    const trend = calculateTrend(81, 80);
    expect(trend.magnitude).toBe('small');
  });

  it('returns medium magnitude for 5-15% change', () => {
    const trend = calculateTrend(85, 80);
    expect(trend.magnitude).toBe('medium');
  });

  it('returns large magnitude for >= 15% change', () => {
    const trend = calculateTrend(95, 80);
    expect(trend.magnitude).toBe('large');
  });

  it('handles zero previous score', () => {
    const trend = calculateTrend(50, 0);
    expect(trend.direction).toBe('up');
    expect(trend.delta).toBe(50);
  });
});

describe('TrendIndicator', () => {
  it('shows up icon for upward trend', () => {
    render(
      <TrendIndicator
        trend={{ direction: 'up', magnitude: 'medium', delta: 5 }}
      />
    );

    const indicator = screen.getByTestId('speedscore-trend');
    expect(indicator).toHaveAttribute('data-direction', 'up');
    expect(indicator).toHaveClass('text-emerald-600');
  });

  it('shows down icon for downward trend', () => {
    render(
      <TrendIndicator
        trend={{ direction: 'down', magnitude: 'medium', delta: -5 }}
      />
    );

    const indicator = screen.getByTestId('speedscore-trend');
    expect(indicator).toHaveAttribute('data-direction', 'down');
    expect(indicator).toHaveClass('text-red-600');
  });

  it('shows minus icon for stable trend', () => {
    render(
      <TrendIndicator
        trend={{ direction: 'stable', magnitude: 'small', delta: 0.5 }}
      />
    );

    const indicator = screen.getByTestId('speedscore-trend');
    expect(indicator).toHaveAttribute('data-direction', 'stable');
    expect(indicator).toHaveClass('text-muted-foreground');
  });

  it('includes magnitude data attribute', () => {
    render(
      <TrendIndicator
        trend={{ direction: 'up', magnitude: 'large', delta: 20 }}
      />
    );

    const indicator = screen.getByTestId('speedscore-trend');
    expect(indicator).toHaveAttribute('data-magnitude', 'large');
  });

  it('has accessible aria-label', () => {
    render(
      <TrendIndicator
        trend={{ direction: 'up', magnitude: 'medium', delta: 5.5 }}
      />
    );

    const indicator = screen.getByTestId('speedscore-trend');
    expect(indicator).toHaveAttribute('aria-label', 'Trend: up, 5.5 points');
  });
});

describe('SpeedScoreBadge', () => {
  describe('loading state', () => {
    it('renders loading skeleton', () => {
      render(<SpeedScoreBadge score={null} isLoading={true} />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge).toHaveAttribute('aria-busy', 'true');
      expect(badge).toHaveClass('animate-pulse');
    });
  });

  describe('error state', () => {
    it('renders error state with retry button', () => {
      const onRetry = vi.fn();
      render(
        <SpeedScoreBadge
          score={null}
          error={new Error('Failed to load')}
          onRetry={onRetry}
        />
      );

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge).toHaveAttribute(
        'aria-label',
        'SpeedScore failed to load. Click to retry.'
      );
      expect(screen.getByText('Error')).toBeInTheDocument();

      fireEvent.click(badge);
      expect(onRetry).toHaveBeenCalledTimes(1);
    });
  });

  describe('not benchmarked state', () => {
    it('renders N/A for null score', () => {
      render(<SpeedScoreBadge score={null} />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge).toHaveAttribute('aria-label', 'Not benchmarked');
      expect(screen.getByText('N/A')).toBeInTheDocument();
    });

    it('renders N/A for undefined score', () => {
      render(<SpeedScoreBadge score={undefined} />);
      expect(screen.getByText('N/A')).toBeInTheDocument();
    });

    it('renders N/A for NaN score', () => {
      render(<SpeedScoreBadge score={NaN} />);
      expect(screen.getByText('N/A')).toBeInTheDocument();
    });

    it('renders N/A for Infinity score', () => {
      render(<SpeedScoreBadge score={Infinity} />);
      expect(screen.getByText('N/A')).toBeInTheDocument();
    });
  });

  describe('normal state with score', () => {
    it('displays rounded score', () => {
      render(<SpeedScoreBadge score={85.7} />);
      expect(screen.getByText('86')).toBeInTheDocument();
    });

    it('has correct aria-label with score and level', () => {
      render(<SpeedScoreBadge score={85} />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge).toHaveAttribute(
        'aria-label',
        'SpeedScore: 85 out of 100, Good'
      );
    });

    it('applies excellent styling for score >= 90', () => {
      render(<SpeedScoreBadge score={95} />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge.className).toContain('emerald');
    });

    it('applies good styling for score 70-89', () => {
      render(<SpeedScoreBadge score={80} />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge.className).toContain('sky');
    });

    it('applies average styling for score 50-69', () => {
      render(<SpeedScoreBadge score={60} />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge.className).toContain('amber');
    });

    it('applies below_average styling for score 30-49', () => {
      render(<SpeedScoreBadge score={40} />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge.className).toContain('orange');
    });

    it('applies poor styling for score < 30', () => {
      render(<SpeedScoreBadge score={20} />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge.className).toContain('red');
    });

    it('clamps scores above 100', () => {
      render(<SpeedScoreBadge score={150} />);
      expect(screen.getByText('100')).toBeInTheDocument();
    });

    it('clamps scores below 0', () => {
      render(<SpeedScoreBadge score={-10} />);
      expect(screen.getByText('0')).toBeInTheDocument();
    });
  });

  describe('trend indicator', () => {
    it('shows trend when previousScore is provided', () => {
      render(<SpeedScoreBadge score={85} previousScore={75} />);

      const trend = screen.getByTestId('speedscore-trend');
      expect(trend).toHaveAttribute('data-direction', 'up');
    });

    it('hides trend when showTrend is false', () => {
      render(
        <SpeedScoreBadge score={85} previousScore={75} showTrend={false} />
      );

      expect(screen.queryByTestId('speedscore-trend')).not.toBeInTheDocument();
    });

    it('hides trend when previousScore is null', () => {
      render(<SpeedScoreBadge score={85} previousScore={null} />);

      expect(screen.queryByTestId('speedscore-trend')).not.toBeInTheDocument();
    });

    it('hides trend when previousScore is undefined', () => {
      render(<SpeedScoreBadge score={85} previousScore={undefined} />);

      expect(screen.queryByTestId('speedscore-trend')).not.toBeInTheDocument();
    });
  });

  describe('size variants', () => {
    it('applies sm size classes by default', () => {
      render(<SpeedScoreBadge score={85} />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge.className).toContain('text-[11px]');
    });

    it('applies md size classes', () => {
      render(<SpeedScoreBadge score={85} size="md" />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge.className).toContain('text-xs');
    });

    it('applies lg size classes', () => {
      render(<SpeedScoreBadge score={85} size="lg" />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge.className).toContain('text-sm');
    });
  });

  describe('tooltip with breakdown', () => {
    const mockBreakdown: SpeedScoreBreakdown = {
      cpu_score: 90,
      memory_score: 78,
      disk_score: 83,
      network_score: 88,
      compilation_score: 87,
      measured_at: '2026-01-17T10:00:00Z',
    };

    it('wraps in Tooltip when breakdown is provided', () => {
      render(<SpeedScoreBadge score={85} breakdown={mockBreakdown} />);

      const wrapper = screen.getByTestId('tooltip-wrapper');
      expect(wrapper).toBeInTheDocument();

      const content = wrapper.getAttribute('data-content');
      expect(content).toContain('SpeedScore: 85/100');
      expect(content).toContain('CPU: 90');
      expect(content).toContain('Memory: 78');
      expect(content).toContain('Disk: 83');
      expect(content).toContain('Network: 88');
      expect(content).toContain('Compilation: 87');
    });

    it('shows relative time in tooltip', () => {
      render(<SpeedScoreBadge score={85} breakdown={mockBreakdown} />);

      const wrapper = screen.getByTestId('tooltip-wrapper');
      const content = wrapper.getAttribute('data-content');
      expect(content).toContain('Last:');
    });

    it('uses title attribute when no breakdown', () => {
      render(<SpeedScoreBadge score={85} />);

      // Should not be wrapped in Tooltip
      expect(screen.queryByTestId('tooltip-wrapper')).not.toBeInTheDocument();

      // Badge's parent span should have title
      const badge = screen.getByTestId('speedscore-badge');
      const parent = badge.parentElement;
      expect(parent?.getAttribute('title')).toContain('SpeedScore 85');
    });
  });

  describe('accessibility', () => {
    it('has role="status" for score display', () => {
      render(<SpeedScoreBadge score={85} />);

      const badge = screen.getByTestId('speedscore-badge');
      expect(badge).toHaveAttribute('role', 'status');
    });

    it('uses tabular-nums for consistent number width', () => {
      render(<SpeedScoreBadge score={85} />);

      const scoreText = screen.getByText('85');
      expect(scoreText).toHaveClass('tabular-nums');
    });
  });
});
