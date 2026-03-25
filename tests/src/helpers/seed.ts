// Vitest setupFiles module — re-executed per test file to register beforeAll hooks.
//
// Reads the client_id / client_secret written by globalSetup and sets them on
// globalThis so the api client can pick them up for every request.

import { readFileSync } from 'node:fs'
import { resolve }      from 'node:path'
import { setCredentials } from './client'

export { ctx, type TestCtx } from './ctx'

function loadDotEnvTest(): void {
  try {
    const content = readFileSync(resolve(process.cwd(), '.env.test'), 'utf-8')
    for (const line of content.split('\n')) {
      const trimmed = line.trim()
      if (!trimmed || trimmed.startsWith('#')) continue
      const eq = trimmed.indexOf('=')
      if (eq < 0) continue
      const key   = trimmed.slice(0, eq).trim()
      const value = trimmed.slice(eq + 1).trim()
      if (key && !process.env[key]) process.env[key] = value
    }
  } catch {
    // Running in Docker — environment variables injected by Compose.
  }
}

loadDotEnvTest()

beforeAll(() => {
  try {
    const raw = readFileSync(resolve(process.cwd(), '.vitest-ctx.json'), 'utf-8')
    const ctx = JSON.parse(raw) as { clientId?: string; clientSecret?: string }
    if (ctx.clientId && ctx.clientSecret) {
      setCredentials(ctx.clientId, ctx.clientSecret)
    } else {
      throw new Error('ctx file is missing clientId or clientSecret')
    }
  } catch (e) {
    throw new Error(
      `Could not load test app credentials from .vitest-ctx.json.\n` +
      `Ensure globalSetup ran successfully.\nDetails: ${e}`,
    )
  }
})
