import { api } from './helpers/client'
import { ctx } from './helpers/seed'

describe('App permissions', () => {
  it('creates an application permission', async () => {
    const { status, body } = await api.post<{
      id: string
      key: string
      name: string
    }>('/permissions', {
      key: 'org:invite',
      name: 'Invite members',
      description: 'Create organization invites',
    })

    expect(status).toBe(200)
    expect(body.key).toBe('org:invite')
    ctx.permissionId = body.id
  })

  it('lists application permissions', async () => {
    const { status, body } = await api.get<Array<{ key: string }>>('/permissions')
    expect(status).toBe(200)
    expect(body.some((p) => p.key === 'org:invite')).toBe(true)
  })

  it('returns 409 for duplicate permission keys', async () => {
    const { status } = await api.post('/permissions', {
      key: 'org:invite',
      name: 'Duplicate',
    })
    expect(status).toBe(409)
  })
})
