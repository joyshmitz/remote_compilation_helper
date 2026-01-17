'use client';

import { Skeleton } from '@/components/ui/skeleton';

export function StatCardSkeleton() {
  return (
    <div
      className="bg-card border border-border rounded-lg p-4"
      data-testid="stat-card-skeleton"
    >
      <div className="flex items-center justify-between mb-2">
        <Skeleton className="h-4 w-20" />
        <Skeleton className="h-4 w-4 rounded-sm" />
      </div>
      <div className="flex items-end gap-2">
        <Skeleton className="h-7 w-16" />
      </div>
    </div>
  );
}
