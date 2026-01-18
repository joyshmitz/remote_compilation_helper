import { NextResponse } from 'next/server';
import { requestRchd } from '@/lib/rchd-client';

export const runtime = 'nodejs';

function jsonResponse(
  data: unknown,
  status: number,
  requestId: string,
  extraHeaders?: Record<string, string>
) {
  return NextResponse.json(data, {
    status,
    headers: {
      'X-Request-ID': requestId,
      ...extraHeaders,
    },
  });
}

export async function GET(
  req: Request,
  { params }: { params: Promise<{ id: string }> }
) {
  const requestId = crypto.randomUUID();
  const { id: workerId } = await params;
  const url = new URL(req.url);
  const days = url.searchParams.get('days');
  const limit = url.searchParams.get('limit');
  const offset = url.searchParams.get('offset');

  const query = new URLSearchParams();
  if (days) query.set('days', days);
  if (limit) query.set('limit', limit);
  if (offset) query.set('offset', offset);

  const path =
    query.toString().length > 0
      ? `/speedscore/${encodeURIComponent(workerId)}/history?${query.toString()}`
      : `/speedscore/${encodeURIComponent(workerId)}/history`;

  try {
    const response = await requestRchd(path);
    const data = response.body ? JSON.parse(response.body) : {};

    if (data?.error === 'worker_not_found') {
      return jsonResponse(data, 404, requestId);
    }
    if (data?.error === 'internal_error') {
      return jsonResponse({ ...data, request_id: requestId }, 500, requestId);
    }

    return jsonResponse(data, 200, requestId);
  } catch (error) {
    return jsonResponse(
      {
        error: 'rchd_unavailable',
        message: error instanceof Error ? error.message : 'Failed to connect to rchd',
        request_id: requestId,
      },
      503,
      requestId
    );
  }
}
