'use client';

import { useState } from 'react';
import useSWR from 'swr';
import { Settings, Server, Clock, Layers, Terminal } from 'lucide-react';
import { api } from '@/lib/api';
import { Skeleton } from '@/components/ui/skeleton';
import { ErrorState, errorHints } from '@/components/ui/error-state';
import type { StatusResponse } from '@/lib/types';

function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  const parts = [];
  if (days > 0) parts.push(`${days}d`);
  if (hours > 0) parts.push(`${hours}h`);
  if (minutes > 0) parts.push(`${minutes}m`);
  if (secs > 0 || parts.length === 0) parts.push(`${secs}s`);

  return parts.join(' ');
}

function SettingsPageSkeleton() {
  return (
    <div className="space-y-6" data-testid="settings-skeleton">
      <div>
        <div className="flex items-center gap-2">
          <Skeleton className="h-6 w-6 rounded-full" />
          <Skeleton className="h-6 w-24" />
        </div>
        <Skeleton className="h-4 w-56 mt-2" />
      </div>

      {Array.from({ length: 4 }).map((_, index) => (
        <div key={`settings-card-${index}`} className="bg-surface border border-border rounded-lg p-6">
          <Skeleton className="h-5 w-40 mb-4" />
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            {Array.from({ length: 4 }).map((_, itemIndex) => (
              <div key={`settings-item-${index}-${itemIndex}`} className="space-y-2">
                <Skeleton className="h-3 w-20" />
                <Skeleton className="h-4 w-32" />
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

export default function SettingsPage() {
  const [isRetrying, setIsRetrying] = useState(false);
  const { data, error, isLoading, mutate } = useSWR<StatusResponse>(
    'status',
    () => api.getStatus(),
    {
      refreshInterval: 5000,
      revalidateOnFocus: true,
    }
  );

  const handleRetry = async () => {
    setIsRetrying(true);
    try {
      await mutate();
    } finally {
      setIsRetrying(false);
    }
  };

  if (isLoading) {
    return <SettingsPageSkeleton />;
  }

  if (error || !data) {
    return (
      <div className="flex items-center justify-center h-full">
        <ErrorState
          error={error || 'Failed to load settings'}
          title="Failed to connect to daemon"
          hint={errorHints.daemonConnection}
          onRetry={handleRetry}
          isRetrying={isRetrying}
        />
      </div>
    );
  }

  const { daemon } = data;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold flex items-center gap-2">
          <Settings className="w-6 h-6" />
          Settings
        </h1>
        <p className="text-muted-foreground text-sm">
          Daemon configuration and status
        </p>
      </div>

      {/* Daemon Info */}
      <div className="bg-surface border border-border rounded-lg p-6">
        <h2 className="text-lg font-semibold mb-4">Daemon Information</h2>
        <div className="grid gap-4 md:grid-cols-2">
          <div className="flex items-start gap-3">
            <Server className="w-5 h-5 text-muted-foreground mt-0.5" />
            <div>
              <div className="text-sm text-muted-foreground">Version</div>
              <div className="font-mono">{daemon.version}</div>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <Terminal className="w-5 h-5 text-muted-foreground mt-0.5" />
            <div>
              <div className="text-sm text-muted-foreground">Process ID</div>
              <div className="font-mono">{daemon.pid}</div>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <Clock className="w-5 h-5 text-muted-foreground mt-0.5" />
            <div>
              <div className="text-sm text-muted-foreground">Uptime</div>
              <div className="font-mono">{formatUptime(daemon.uptime_secs)}</div>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <Layers className="w-5 h-5 text-muted-foreground mt-0.5" />
            <div>
              <div className="text-sm text-muted-foreground">Socket Path</div>
              <div className="font-mono text-sm break-all">{daemon.socket_path}</div>
            </div>
          </div>
        </div>
      </div>

      {/* Fleet Summary */}
      <div className="bg-surface border border-border rounded-lg p-6">
        <h2 className="text-lg font-semibold mb-4">Fleet Summary</h2>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
          <div>
            <div className="text-sm text-muted-foreground">Total Workers</div>
            <div className="text-2xl font-bold">{daemon.workers_total}</div>
          </div>
          <div>
            <div className="text-sm text-muted-foreground">Healthy Workers</div>
            <div className="text-2xl font-bold text-healthy">{daemon.workers_healthy}</div>
          </div>
          <div>
            <div className="text-sm text-muted-foreground">Total Slots</div>
            <div className="text-2xl font-bold">{daemon.slots_total}</div>
          </div>
          <div>
            <div className="text-sm text-muted-foreground">Available Slots</div>
            <div className="text-2xl font-bold text-primary">{daemon.slots_available}</div>
          </div>
        </div>
      </div>

      {/* Configuration Files */}
      <div className="bg-surface border border-border rounded-lg p-6">
        <h2 className="text-lg font-semibold mb-4">Configuration</h2>
        <div className="space-y-3 text-sm">
          <div>
            <div className="text-muted-foreground mb-1">Workers Config</div>
            <code className="bg-surface-elevated px-2 py-1 rounded text-xs">
              ~/.config/rch/workers.toml
            </code>
          </div>
          <div>
            <div className="text-muted-foreground mb-1">Daemon Config</div>
            <code className="bg-surface-elevated px-2 py-1 rounded text-xs">
              ~/.config/rch/daemon.toml
            </code>
          </div>
        </div>
      </div>

      {/* API Endpoints */}
      <div className="bg-surface border border-border rounded-lg p-6">
        <h2 className="text-lg font-semibold mb-4">API Endpoints</h2>
        <div className="space-y-2 text-sm font-mono">
          <div className="flex gap-2">
            <span className="text-muted-foreground">GET</span>
            <span>/health</span>
            <span className="text-muted-foreground ml-auto">Basic health check</span>
          </div>
          <div className="flex gap-2">
            <span className="text-muted-foreground">GET</span>
            <span>/ready</span>
            <span className="text-muted-foreground ml-auto">Readiness probe</span>
          </div>
          <div className="flex gap-2">
            <span className="text-muted-foreground">GET</span>
            <span>/metrics</span>
            <span className="text-muted-foreground ml-auto">Prometheus metrics</span>
          </div>
          <div className="flex gap-2">
            <span className="text-muted-foreground">GET</span>
            <span>/budget</span>
            <span className="text-muted-foreground ml-auto">Budget compliance</span>
          </div>
        </div>
      </div>
    </div>
  );
}
