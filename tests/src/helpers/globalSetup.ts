// Vitest globalSetup — runs once in the main process before any test files.
//
// 1. Wipes the ctx file so IDs from a previous run don't leak.
// 2. Logs in as the bootstrap admin user (created by the API entrypoint on startup).
// 3. Creates a fresh test application via the admin JSON API and writes its
//    client_id + client_secret to the ctx file so test files can pick them up.

import { existsSync, unlinkSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const CTX_FILE      = join(process.cwd(), '.vitest-ctx.json')
const BASE_URL      = process.env.API_URL            ?? 'http://localhost:8080'
const ADMIN_EMAIL   = process.env.AUTHSTACK_ADMIN_EMAIL  ?? 'test-admin@authstack.local'
const ADMIN_PASSWORD = process.env.AUTHSTACK_ADMIN_PASSWORD ?? 'test-admin-password-123'

export async function setup(): Promise<void> {
  if (existsSync(CTX_FILE)) unlinkSync(CTX_FILE)

  const loginRes = await fetch(`${BASE_URL}/admin/login`, {
    method:   'POST',
    headers:  { 'Content-Type': 'application/x-www-form-urlencoded' },
    body:     new URLSearchParams({ email: ADMIN_EMAIL, password: ADMIN_PASSWORD }).toString(),
    redirect: 'manual',
  })
  if (loginRes.status !== 303) {
    const text = await loginRes.text()
    throw new Error(
      `Admin login failed (${loginRes.status}): ${text}\n` +
      `Is the API running at ${BASE_URL}? Was the bootstrap admin created on startup?\n` +
      `Check AUTHSTACK_BOOTSTRAP_EMAIL and AUTHSTACK_BOOTSTRAP_PASSWORD on the API container.`,
    )
  }
  const rawCookie = loginRes.headers.get('set-cookie')
  if (!rawCookie) throw new Error('Admin login did not return a Set-Cookie header.')
  const adminCookie = rawCookie.split(';')[0]

  const appRes = await fetch(`${BASE_URL}/admin/applications`, {
    method:  'POST',
    headers: {
      'Content-Type': 'application/json',
      'Cookie':        adminCookie,
    },
    body: JSON.stringify({ name: 'test-app' }),
  })
  if (!appRes.ok) {
    const text = await appRes.text()
    throw new Error(`Failed to create test application (${appRes.status}): ${text}`)
  }

  const app = await appRes.json() as { id: string; client_secret: string }
  writeFileSync(CTX_FILE, JSON.stringify({ clientId: app.id, clientSecret: app.client_secret }, null, 2))
}
