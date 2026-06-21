// Credentials are stored on globalThis so they survive Vitest's per-file
// module isolation when running with pool: 'forks' + singleFork: true.
declare global {
  var __clientId: string | undefined
  var __clientSecret: string | undefined
}

const BASE_URL = process.env.API_URL ?? 'http://localhost:8080'

export function setCredentials(clientId: string, clientSecret: string): void {
  globalThis.__clientId = clientId
  globalThis.__clientSecret = clientSecret
}

export function getCredentials(): { clientId: string; clientSecret: string } | null {
  if (!globalThis.__clientId || !globalThis.__clientSecret) return null
  return { clientId: globalThis.__clientId, clientSecret: globalThis.__clientSecret }
}

function basicAuthHeader(): string | null {
  const creds = getCredentials()
  if (!creds) return null
  return 'Basic ' + Buffer.from(`${creds.clientId}:${creds.clientSecret}`).toString('base64')
}

async function request<T = unknown>(
  method: string,
  path: string,
  body?: unknown,
  extraHeaders?: Record<string, string>,
): Promise<{ status: number; body: T }> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' }

  const auth = basicAuthHeader()
  if (auth) headers['Authorization'] = auth

  if (extraHeaders) Object.assign(headers, extraHeaders)

  const res = await fetch(`${BASE_URL}${path}`, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  })

  const text = await res.text()
  let parsed: T
  try {
    parsed = text ? (JSON.parse(text) as T) : (null as T)
  } catch {
    parsed = text as unknown as T
  }

  return { status: res.status, body: parsed }
}

export const api = {
  get:    <T = unknown>(path: string, headers?: Record<string, string>) => request<T>('GET', path, undefined, headers),
  post:   <T = unknown>(path: string, body?: unknown, headers?: Record<string, string>) => request<T>('POST', path, body, headers),
  put:    <T = unknown>(path: string, body?: unknown) => request<T>('PUT', path, body),
  patch:  <T = unknown>(path: string, body?: unknown) => request<T>('PATCH', path, body),
  delete: <T = unknown>(path: string) => request<T>('DELETE', path),
}
