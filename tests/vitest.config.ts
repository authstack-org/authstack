import { sep }          from 'node:path'
import { defineConfig }  from 'vitest/config'

// Test files must run in this order — later suites depend on IDs created in earlier ones.
const FILE_ORDER = [
  'src/admin.test.ts',
  'src/auth.test.ts',
  'src/users.test.ts',
  'src/orgs.test.ts',
  'src/members.test.ts',
]

class OrderedSequencer {
  async sort(files: [unknown, string][]): Promise<[unknown, string][]> {
    return [...files].sort((a, b) => {
      const norm  = (p: string) => p.replaceAll('/', sep).replaceAll('\\', sep)
      const idxOf = (f: [unknown, string]) =>
        FILE_ORDER.findIndex((p) => f[1].endsWith(norm(p)))
      const ai = idxOf(a); const bi = idxOf(b)
      return (ai === -1 ? FILE_ORDER.length : ai) - (bi === -1 ? FILE_ORDER.length : bi)
    })
  }
  async shard(files: [unknown, string][]): Promise<[unknown, string][]> {
    return files
  }
}

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    globalSetup: ['./src/helpers/globalSetup.ts'],
    setupFiles:  ['./src/helpers/seed.ts'],
    pool: 'forks',
    poolOptions: { forks: { singleFork: true } },
    sequence: { sequencer: OrderedSequencer as any },
    include: FILE_ORDER,
  },
})
