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
  _req: Request,
  { params }: { params: Promise<{ id: string }> }
) {
  const requestId = crypto.randomUUID();
  const { id: workerId } = await params;

  try {
    const response = await requestRchd(`/speedscore/${encodeURIComponent(workerId)}`);
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
