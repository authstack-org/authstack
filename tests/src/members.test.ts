import { api } from './helpers/client'
import { ctx } from './helpers/seed'

describe('Members', () => {
  it('adds a member to an organization', async () => {
    const { status } = await api.post(`/orgs/${ctx.orgId}/members`, {
      user_id: ctx.userId,
      role:    'member',
    })
    expect(status).toBe(200)
  })

  it('lists members of an organization', async () => {
    const { status, body } = await api.get<Array<{ user_id: string; role: string }>>(
      `/orgs/${ctx.orgId}/members`,
    )
    expect(status).toBe(200)
    expect(body.some((m) => m.user_id === ctx.userId && m.role === 'member')).toBe(true)
  })

  it('removes a member from an organization', async () => {
    const { status } = await api.delete(`/orgs/${ctx.orgId}/members/${ctx.userId}`)
    expect(status).toBe(200)
  })
})
