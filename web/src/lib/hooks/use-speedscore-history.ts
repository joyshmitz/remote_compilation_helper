'use client';

import { useQuery, useInfiniteQuery } from '@tanstack/react-query';
import { api } from '../api';
import type { SpeedScoreHistoryResponse, SpeedScoreView } from '../types';

interface UseSpeedScoreHistoryOptions {
  /** Number of records per page (default: 100) */
  limit?: number;
  /** Enable automatic refetching */
  refetchInterval?: number;
  /** Enable the query */
  enabled?: boolean;
}

/**
 * Hook for fetching SpeedScore history for a worker.
 * Supports pagination via useInfiniteQuery for large datasets.
 */
export function useSpeedScoreHistory(
  workerId: string | null | undefined,
  options: UseSpeedScoreHistoryOptions = {}
) {
  const { limit = 100, refetchInterval, enabled = true } = options;

  return useInfiniteQuery<SpeedScoreHistoryResponse>({
    queryKey: ['speedscore-history', workerId, limit],
    queryFn: async ({ pageParam = 0 }) => {
      if (!workerId) {
        throw new Error('Worker ID is required');
      }
      return api.getSpeedScoreHistory(workerId, {
        limit,
        offset: pageParam as number,
      });
    },
    initialPageParam: 0,
    getNextPageParam: (lastPage) => {
      if (!lastPage.pagination.has_more) return undefined;
      return lastPage.pagination.offset + lastPage.pagination.limit;
    },
    enabled: enabled && !!workerId,
    refetchInterval,
  });
}

/**
 * Hook for fetching a single page of SpeedScore history.
 * Simpler alternative when pagination isn't needed.
 */
export function useSpeedScoreHistoryPage(
  workerId: string | null | undefined,
  options: UseSpeedScoreHistoryOptions = {}
) {
  const { limit = 100, refetchInterval, enabled = true } = options;

  return useQuery<SpeedScoreHistoryResponse>({
    queryKey: ['speedscore-history-page', workerId, limit],
    queryFn: () => {
      if (!workerId) {
        throw new Error('Worker ID is required');
      }
      return api.getSpeedScoreHistory(workerId, { limit });
    },
    enabled: enabled && !!workerId,
    refetchInterval,
  });
}

/**
 * Flatten paginated history into a single array.
 */
export function flattenHistory(
  pages: SpeedScoreHistoryResponse[] | undefined
): SpeedScoreView[] {
  if (!pages) return [];
  return pages.flatMap((page) => page.history);
}
