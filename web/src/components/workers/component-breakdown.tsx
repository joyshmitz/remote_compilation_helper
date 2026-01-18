'use client';

import type { SpeedScoreView, BenchmarkResults, PartialSpeedScore } from '@/lib/types';
import { ComponentRow } from './component-row';

type SpeedScoreKey = 'cpu' | 'memory' | 'disk' | 'network' | 'compilation';

interface ComponentInfo {
  name: string;
  weight: number;
  description: string;
  formatRaw: (raw: BenchmarkResults[SpeedScoreKey] | undefined) => string;
}

/**
 * Component configuration with weights and formatting.
 * Weights must sum to 1.0 (100%).
 */
const COMPONENT_INFO: Record<SpeedScoreKey, ComponentInfo> = {
  cpu: {
    name: 'CPU',
    weight: 0.30,
    description: 'Floating-point computation throughput. Higher is better for compute-heavy compilation.',
    formatRaw: (raw) => {
      const cpuRaw = raw as BenchmarkResults['cpu'];
      return cpuRaw?.gflops != null ? `${cpuRaw.gflops.toFixed(1)} GFLOPS` : 'N/A';
    },
  },
  memory: {
    name: 'Memory',
    weight: 0.15,
    description: 'Memory bandwidth for large projects with many compilation units.',
    formatRaw: (raw) => {
      const memRaw = raw as BenchmarkResults['memory'];
      return memRaw?.bandwidth_gbps != null ? `${memRaw.bandwidth_gbps.toFixed(1)} GB/s` : 'N/A';
    },
  },
  disk: {
    name: 'Disk',
    weight: 0.20,
    description: 'Storage performance affecting source file reads and artifact writes.',
    formatRaw: (raw) => {
      const diskRaw = raw as BenchmarkResults['disk'];
      if (!diskRaw) return 'N/A';
      const seqRead = diskRaw.sequential_read_mbps?.toFixed(0) ?? '?';
      const iops = diskRaw.random_read_iops ?? '?';
      return `${seqRead} MB/s seq, ${iops} IOPS`;
    },
  },
  network: {
    name: 'Network',
    weight: 0.15,
    description: 'File transfer throughput between your machine and this worker.',
    formatRaw: (raw) => {
      const netRaw = raw as BenchmarkResults['network'];
      if (!netRaw) return 'N/A';
      const down = netRaw.download_mbps?.toFixed(0) ?? '?';
      const up = netRaw.upload_mbps?.toFixed(0) ?? '?';
      return `${down}↓/${up}↑ Mbps`;
    },
  },
  compilation: {
    name: 'Compilation',
    weight: 0.20,
    description: 'Actual Rust compilation speed using a reference project.',
    formatRaw: (raw) => {
      const compRaw = raw as BenchmarkResults['compilation'];
      return compRaw?.units_per_sec != null ? `${compRaw.units_per_sec.toFixed(1)} units/sec` : 'N/A';
    },
  },
};

const COMPONENT_ORDER: SpeedScoreKey[] = ['cpu', 'memory', 'disk', 'network', 'compilation'];

export interface ComponentBreakdownProps {
  speedscore: SpeedScoreView | PartialSpeedScore;
  rawResults?: BenchmarkResults | null;
  partialUpTo?: SpeedScoreKey;
}

/**
 * ComponentBreakdown displays all five SpeedScore components with their
 * individual scores, weights, contributions, and raw benchmark values.
 */
export function ComponentBreakdown({
  speedscore,
  rawResults,
  partialUpTo,
}: ComponentBreakdownProps) {
  const partialIndex = partialUpTo ? COMPONENT_ORDER.indexOf(partialUpTo) : -1;

  return (
    <div className="space-y-1">
      {/* Header row (hidden on mobile) */}
      <div className="hidden sm:grid grid-cols-[100px_1fr_48px_56px_56px_24px_120px] gap-2 px-1 py-1 text-xs text-muted-foreground font-medium border-b border-border">
        <div>Component</div>
        <div>Score</div>
        <div className="text-right">Value</div>
        <div className="text-right">Weight</div>
        <div className="text-right">Contrib</div>
        <div></div>
        <div>Raw</div>
      </div>

      {/* Component rows */}
      {COMPONENT_ORDER.map((key, index) => {
        const info = COMPONENT_INFO[key];
        const scoreKey = `${key}_score` as keyof SpeedScoreView;
        const score = speedscore[scoreKey] as number | undefined;
        const raw = rawResults?.[key];
        const isPartial = partialIndex >= 0 && index > partialIndex;

        return (
          <ComponentRow
            key={key}
            name={info.name}
            score={isPartial ? null : (score ?? null)}
            weight={info.weight}
            rawValue={isPartial ? 'Not measured' : info.formatRaw(raw)}
            description={info.description}
            isPartial={isPartial}
            data-testid={`component-${key}`}
          />
        );
      })}

      {/* Total calculation */}
      <div className="flex items-center justify-between pt-3 mt-2 border-t border-border">
        <span className="text-sm text-muted-foreground">
          Total = Σ(score × weight)
        </span>
        <span className="text-lg font-semibold tabular-nums">
          {'total' in speedscore && speedscore.total != null
            ? speedscore.total.toFixed(1)
            : '—'}
        </span>
      </div>
    </div>
  );
}
