import type {
  StatusResponse,
  HealthResponse,
  ReadyResponse,
  BudgetStatusResponse,
  SpeedScoreHistoryResponse,
} from './types';

// Default to local daemon socket proxy
const API_BASE = process.env.NEXT_PUBLIC_RCH_API_URL || 'http://localhost:9100';

class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public data?: unknown
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function fetchApi<T>(endpoint: string): Promise<T> {
  try {
    const response = await fetch(`${API_BASE}${endpoint}`, {
      headers: {
        'Accept': 'application/json',
      },
    });

    if (!response.ok) {
      const text = await response.text();
      throw new ApiError(
        `API request failed: ${response.statusText}`,
        response.status,
        text
      );
    }

    return response.json() as T;
  } catch (error) {
    if (error instanceof ApiError) {
      throw error;
    }
    // Network error or daemon offline
    throw new ApiError(
      error instanceof Error ? error.message : 'Failed to connect to daemon',
      0
    );
  }
}

export const api = {
  /**
   * Get full daemon status including workers, builds, and issues
   */
  async getStatus(): Promise<StatusResponse> {
    return fetchApi<StatusResponse>('/status');
  },

  /**
   * Get basic health check
   */
  async getHealth(): Promise<HealthResponse> {
    return fetchApi<HealthResponse>('/health');
  },

  /**
   * Get readiness status
   */
  async getReady(): Promise<ReadyResponse> {
    return fetchApi<ReadyResponse>('/ready');
  },

  /**
   * Get performance budget status
   */
  async getBudget(): Promise<BudgetStatusResponse> {
    return fetchApi<BudgetStatusResponse>('/budget');
  },

  /**
   * Get Prometheus metrics (text format)
   */
  async getMetrics(): Promise<string> {
    const response = await fetch(`${API_BASE}/metrics`, {
      headers: {
        'Accept': 'text/plain',
      },
    });
    return response.text();
  },

  /**
   * Get SpeedScore history for a worker with pagination
   */
  async getSpeedScoreHistory(
    workerId: string,
    options?: { limit?: number; offset?: number }
  ): Promise<SpeedScoreHistoryResponse> {
    const params = new URLSearchParams();
    if (options?.limit) params.set('limit', String(options.limit));
    if (options?.offset) params.set('offset', String(options.offset));
    const query = params.toString() ? `?${params}` : '';
    return fetchApi<SpeedScoreHistoryResponse>(`/api/speedscore/${workerId}/history${query}`);
  },
};

export { ApiError };
