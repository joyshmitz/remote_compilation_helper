'use client';

import { useState } from 'react';
import useSWR from 'swr';
import { Activity, RefreshCw, CheckCircle, XCircle, AlertCircle } from 'lucide-react';
import { motion } from 'motion/react';
import { api } from '@/lib/api';
import { Progress } from '@/components/ui/progress';
import { Skeleton } from '@/components/ui/skeleton';
import { ErrorState, errorHints } from '@/components/ui/error-state';
import type { BudgetStatusResponse } from '@/lib/types';

function MetricsPageSkeleton() {
  return (
    <div className="space-y-6" data-testid="metrics-skeleton">
      <div className="flex items-center justify-between">
        <div>
          <div className="flex items-center gap-2">
            <Skeleton className="h-6 w-6 rounded-full" />
            <Skeleton className="h-6 w-24" />
          </div>
          <Skeleton className="h-4 w-60 mt-2" />
        </div>
        <Skeleton className="h-9 w-9 rounded-lg" />
      </div>

      <div>
        <div className="flex items-center gap-2 mb-3">
          <Skeleton className="h-5 w-36" />
          <Skeleton className="h-5 w-20 rounded-full" />
        </div>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 3 }).map((_, index) => (
            <div
              key={`budget-skeleton-${index}`}
              className="bg-surface border border-border rounded-lg p-4"
            >
              <div className="flex items-center justify-between mb-2">
                <Skeleton className="h-4 w-28" />
                <Skeleton className="h-4 w-4 rounded-full" />
              </div>
              <div className="space-y-2">
                {Array.from({ length: 3 }).map((_, rowIndex) => (
                  <div key={`budget-row-${rowIndex}`} className="flex justify-between">
                    <Skeleton className="h-3 w-14" />
                    <Skeleton className="h-3 w-16" />
                  </div>
                ))}
              </div>
              <div className="mt-3">
                <Skeleton className="h-1 w-full rounded-full" />
              </div>
            </div>
          ))}
        </div>
      </div>

      <div>
        <Skeleton className="h-5 w-40 mb-3" />
        <div className="bg-surface border border-border rounded-lg p-4">
          <div className="space-y-2">
            {Array.from({ length: 6 }).map((_, index) => (
              <Skeleton key={`metric-line-${index}`} className="h-3 w-full" />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

export default function MetricsPage() {
  const [isRetrying, setIsRetrying] = useState(false);
  const { data: budgetData, error: budgetError, isLoading: budgetLoading, mutate, isValidating } = useSWR<BudgetStatusResponse>(
    'budget',
    () => api.getBudget(),
    {
      refreshInterval: 5000, // Poll every 5 seconds
      revalidateOnFocus: true,
    }
  );

  const { data: metricsText, error: metricsError, isLoading: metricsLoading, mutate: mutateMetrics } = useSWR<string>(
    'metrics',
    () => api.getMetrics(),
    {
      refreshInterval: 5000,
      revalidateOnFocus: true,
    }
  );

  const handleRetry = async () => {
    setIsRetrying(true);
    try {
      await Promise.all([mutate(), mutateMetrics()]);
    } finally {
      setIsRetrying(false);
    }
  };

  if (budgetLoading || metricsLoading) {
    return <MetricsPageSkeleton />;
  }

  if (budgetError || metricsError) {
    return (
      <div className="flex items-center justify-center h-full">
        <ErrorState
          error={budgetError || metricsError || 'Failed to load metrics'}
          title="Failed to load metrics"
          hint={errorHints.daemonConnection}
          onRetry={handleRetry}
          isRetrying={isRetrying}
        />
      </div>
    );
  }

  const statusIcon = budgetData?.status === 'passing' ? (
    <CheckCircle className="w-5 h-5 text-healthy" />
  ) : budgetData?.status === 'warning' ? (
    <AlertCircle className="w-5 h-5 text-warning" />
  ) : (
    <XCircle className="w-5 h-5 text-error" />
  );

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Activity className="w-6 h-6" />
            Metrics
          </h1>
          <p className="text-muted-foreground text-sm">
            Performance budgets and Prometheus metrics
          </p>
        </div>
        <button
          type="button"
          onClick={() => mutate()}
          disabled={isValidating}
          className="p-2 rounded-lg hover:bg-surface-elevated transition-colors disabled:opacity-50"
          title="Refresh"
          aria-label="Refresh metrics"
        >
          <motion.div
            animate={isValidating ? { rotate: 360 } : { rotate: 0 }}
            transition={isValidating ? { duration: 1, repeat: Infinity, ease: 'linear' } : {}}
          >
            <RefreshCw className="w-5 h-5 text-muted-foreground" />
          </motion.div>
        </button>
      </div>

      {/* Budget Status */}
      {budgetData && (
        <div>
          <h2 className="text-lg font-semibold mb-3 flex items-center gap-2">
            {statusIcon}
            Performance Budgets
            <span className={`text-sm font-normal px-2 py-0.5 rounded ${
              budgetData.status === 'passing' ? 'bg-healthy/20 text-healthy' :
              budgetData.status === 'warning' ? 'bg-warning/20 text-warning' :
              'bg-error/20 text-error'
            }`}>
              {budgetData.status}
            </span>
          </h2>
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {budgetData.budgets.map((budget) => (
              <div
                key={budget.name}
                className={`bg-surface border rounded-lg p-4 ${
                  budget.is_passing ? 'border-border' : 'border-error/50'
                }`}
              >
                <div className="flex items-center justify-between mb-2">
                  <span className="font-medium">{budget.name}</span>
                  {budget.is_passing ? (
                    <CheckCircle className="w-4 h-4 text-healthy" />
                  ) : (
                    <XCircle className="w-4 h-4 text-error" />
                  )}
                </div>
                <div className="space-y-2 text-sm">
                  <div className="flex justify-between text-muted-foreground">
                    <span>Budget</span>
                    <span className="font-mono">{budget.budget_ms}ms</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">p50</span>
                    <span className="font-mono">{budget.p50_ms.toFixed(2)}ms</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">p95</span>
                    <span className={`font-mono ${budget.p95_ms > budget.budget_ms ? 'text-error' : ''}`}>
                      {budget.p95_ms.toFixed(2)}ms
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">p99</span>
                    <span className={`font-mono ${budget.p99_ms > budget.budget_ms ? 'text-error' : ''}`}>
                      {budget.p99_ms.toFixed(2)}ms
                    </span>
                  </div>
                  {budget.violation_count > 0 && (
                    <div className="flex justify-between text-error">
                      <span>Violations</span>
                      <span className="font-mono">{budget.violation_count}</span>
                    </div>
                  )}
                </div>
                <div className="mt-3">
                  <Progress
                    value={Math.min((budget.p95_ms / budget.budget_ms) * 100, 100)}
                    className="h-1"
                    aria-label={`${budget.name} budget utilization`}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Raw Prometheus Metrics */}
      <div>
        <h2 className="text-lg font-semibold mb-3">Prometheus Metrics</h2>
        <div className="bg-surface border border-border rounded-lg p-4 overflow-auto max-h-96">
          <pre className="text-xs font-mono text-muted-foreground whitespace-pre">
            {metricsText || 'No metrics available'}
          </pre>
        </div>
      </div>
    </div>
  );
}
