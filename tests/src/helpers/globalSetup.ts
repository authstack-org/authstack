// Vitest globalSetup — runs once in the main process before any test files.
//
// 1. Wipes the ctx file so IDs from a previous run don't leak.
// 2. Creates a bootstrap admin user via X-Admin-Key (idempotent — 409 is OK).
// 3. Logs in as that admin user to obtain a session cookie.
// 4. Creates a fresh test application via the admin JSON API and writes its
//    client_id + client_secret to the ctx file so test files can pick them up.

import { existsSync, unlinkSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const CTX_FILE      = join(process.cwd(), '.vitest-ctx.json')
const BASE_URL      = process.env.API_URL            ?? 'http://localhost:8080'
const ADMIN_KEY     = process.env.AUTHSTACK_ADMIN_KEY    ?? 'change_me_in_tests'
const ADMIN_EMAIL   = process.env.AUTHSTACK_ADMIN_EMAIL  ?? 'test-admin@authstack.local'
const ADMIN_PASSWORD = process.env.AUTHSTACK_ADMIN_PASSWORD ?? 'test-admin-password-123'

export async function setup(): Promise<void> {
  if (existsSync(CTX_FILE)) unlinkSync(CTX_FILE)

  // Step 1: create the bootstrap admin user (409 is fine — already exists).
  const createAdminRes = await fetch(`${BASE_URL}/admin/users`, {
    method:  'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Admin-Key':  ADMIN_KEY,
    },
    body: JSON.stringify({ email: ADMIN_EMAIL, password: ADMIN_PASSWORD }),
  })
  if (!createAdminRes.ok && createAdminRes.status !== 409) {
    const text = await createAdminRes.text()
    throw new Error(
      `Failed to create admin user (${createAdminRes.status}): ${text}\n` +
      `Is the API running at ${BASE_URL}? Is AUTHSTACK_ADMIN_KEY correct?`,
    )
  }

  // Step 2: log in as the admin to get a session cookie.
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
      `Check AUTHSTACK_ADMIN_EMAIL and AUTHSTACK_ADMIN_PASSWORD.`,
    )
  }
  const rawCookie = loginRes.headers.get('set-cookie')
  if (!rawCookie) throw new Error('Admin login did not return a Set-Cookie header.')
  // Extract just the name=value part (strip Path, HttpOnly, etc.)
  const adminCookie = rawCookie.split(';')[0]

  // Step 3: create a fresh test application via the JSON API.
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
