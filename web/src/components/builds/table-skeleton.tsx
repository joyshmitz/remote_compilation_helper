'use client';

import { Skeleton } from '@/components/ui/skeleton';
import { cn } from '@/lib/utils';

interface TableSkeletonProps {
  rows?: number;
  columns?: number;
  className?: string;
  testId?: string;
}

export function TableSkeleton({
  rows = 5,
  columns = 5,
  className,
  testId = 'table-skeleton',
}: TableSkeletonProps) {
  return (
    <div
      className={cn('bg-surface border border-border rounded-lg overflow-hidden', className)}
      data-testid={testId}
    >
      <table className="w-full text-sm">
        <thead className="bg-surface-elevated">
          <tr>
            {Array.from({ length: columns }).map((_, index) => (
              <th key={`head-${index}`} className="p-3">
                <Skeleton className="h-3 w-20" />
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {Array.from({ length: rows }).map((_, rowIndex) => (
            <tr key={`row-${rowIndex}`}>
              {Array.from({ length: columns }).map((_, colIndex) => (
                <td key={`cell-${rowIndex}-${colIndex}`} className="p-3">
                  <Skeleton className="h-3 w-full max-w-[140px]" />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
