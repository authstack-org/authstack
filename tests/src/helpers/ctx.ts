// Shared test context backed by a JSON file so mutations persist across
// test files regardless of how Vitest isolates module registries.
//
// The file is created lazily and wiped at the start of each run by
// globalSetup (src/helpers/globalSetup.ts).

import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const CTX_FILE = join(process.cwd(), '.vitest-ctx.json')

export interface TestCtx {
  // Application credentials (created in globalSetup)
  clientId?:    string
  clientSecret?: string
  // Created during tests
  userId?:      string
  orgId?:       string
  permissionId?: string
  memberRoleId?: string
  customRoleId?: string
  accessToken?: string
  refreshToken?: string
}

function readCtx(): Partial<TestCtx> {
  try {
    if (!existsSync(CTX_FILE)) return {}
    return JSON.parse(readFileSync(CTX_FILE, 'utf-8')) as Partial<TestCtx>
  } catch {
    return {}
  }
}

function writeCtx(data: Partial<TestCtx>): void {
  writeFileSync(CTX_FILE, JSON.stringify(data, null, 2))
}

export const ctx = new Proxy({} as TestCtx, {
  get(_t, key: string) {
    return readCtx()[key as keyof TestCtx]
  },
  set(_t, key: string, value: unknown) {
    const data = readCtx()
    ;(data as Record<string, unknown>)[key] = value
    writeCtx(data)
    return true
  },
})
