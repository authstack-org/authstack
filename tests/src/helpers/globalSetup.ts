// Vitest globalSetup — runs once in the main process before any test files.
//
// 1. Wipes the ctx file so IDs from a previous run don't leak.
// 2. Creates a fresh test application via the admin API and writes its
//    client_id + client_secret to the ctx file so test files can pick them up.

import { existsSync, unlinkSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const CTX_FILE  = join(process.cwd(), '.vitest-ctx.json')
const BASE_URL  = process.env.API_URL   ?? 'http://localhost:8080'
const ADMIN_KEY = process.env.AEGIS_ADMIN_KEY ?? 'change_me_in_tests'

export async function setup(): Promise<void> {
  if (existsSync(CTX_FILE)) unlinkSync(CTX_FILE)

  const res = await fetch(`${BASE_URL}/admin/applications`, {
    method:  'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Admin-Key':  ADMIN_KEY,
    },
    body: JSON.stringify({ name: 'test-app' }),
  })

  if (!res.ok) {
    const text = await res.text()
    throw new Error(
      `Failed to create test application (${res.status}): ${text}\n` +
      `Is the API running at ${BASE_URL}? Is AEGIS_ADMIN_KEY correct?`,
    )
  }

  const app = await res.json() as { client_id: string; client_secret: string }
  writeFileSync(CTX_FILE, JSON.stringify({ clientId: app.client_id, clientSecret: app.client_secret }, null, 2))
}
