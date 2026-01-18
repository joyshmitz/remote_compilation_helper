import net from 'node:net';
import path from 'node:path';
import os from 'node:os';

export type RchdMethod = 'GET' | 'POST';

export interface RchdResponse {
  statusLine: string;
  headers: Record<string, string>;
  body: string;
}

function defaultSocketPath(): string {
  const runtimeDir = process.env.XDG_RUNTIME_DIR;
  if (runtimeDir && runtimeDir.trim().length > 0) {
    return path.join(runtimeDir, 'rch.sock');
  }

  const home = process.env.HOME || os.homedir();
  if (home) {
    return path.join(home, '.cache', 'rch', 'rch.sock');
  }

  return '/tmp/rch.sock';
}

function resolveSocketPath(): string {
  return (
    process.env.RCH_SOCKET_PATH ||
    process.env.RCH_DAEMON_SOCKET ||
    defaultSocketPath()
  );
}

function parseResponse(raw: string): RchdResponse {
  const [headerPart, ...bodyParts] = raw.split('\r\n\r\n');
  const body = bodyParts.join('\r\n\r\n').trim();
  const headerLines = headerPart.split('\r\n');
  const statusLine = headerLines.shift() || '';
  const headers: Record<string, string> = {};
  for (const line of headerLines) {
    const idx = line.indexOf(':');
    if (idx === -1) continue;
    const key = line.slice(0, idx).trim().toLowerCase();
    const value = line.slice(idx + 1).trim();
    headers[key] = value;
  }
  return { statusLine, headers, body };
}

export async function requestRchd(
  path: string,
  options: { method?: RchdMethod; body?: string } = {}
): Promise<RchdResponse> {
  const socketPath = resolveSocketPath();
  const method = options.method ?? 'GET';
  const requestLine = `${method} ${path} HTTP/1.0\r\n\r\n`;

  return new Promise((resolve, reject) => {
    const client = net.createConnection({ path: socketPath }, () => {
      client.write(requestLine);
      if (options.body) {
        client.write(options.body);
      }
      client.end();
    });

    let data = '';
    client.setEncoding('utf8');

    client.on('data', (chunk) => {
      data += chunk;
    });

    client.on('end', () => {
      resolve(parseResponse(data));
    });

    client.on('error', (err) => {
      reject(err);
    });
  });
}

export function getSocketPath(): string {
  return resolveSocketPath();
}
