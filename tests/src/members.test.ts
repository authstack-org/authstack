import { api } from './helpers/client'
import { ctx } from './helpers/seed'

describe('Members', () => {
  it('rejects adding a member to a personal organization', async () => {
    const { status: listStatus, body: orgs } = await api.get<
      Array<{ id: string; org_type: string }>
    >('/orgs')
    expect(listStatus).toBe(200)
    const personal = orgs.find((o) => o.org_type === 'personal')
    expect(personal).toBeTruthy()

    const { status } = await api.post(`/orgs/${personal!.id}/members`, {
      user_id: ctx.userId,
      role: 'member',
    })
    expect(status).toBe(422)
  })

  it('lists members of the team org — empty initially', async () => {
    const { status, body } = await api.get<Array<{ user_id: string }>>(`/orgs/${ctx.orgId}/members`)
    expect(status).toBe(200)
    expect(Array.isArray(body)).toBe(true)
    expect(body).toHaveLength(0)
  })

  it('adds the signed-up user to the team org', async () => {
    const { status, body } = await api.post<{
      id: string
      user_id: string
      organization_id: string
      role: string
    }>(`/orgs/${ctx.orgId}/members`, { user_id: ctx.userId, role: 'member' })

    expect(status).toBe(200)
    expect(body.user_id).toBe(ctx.userId)
    expect(body.organization_id).toBe(ctx.orgId)
    expect(body.role).toBe('member')
  })

  it('returns 409 when adding the same user twice', async () => {
    const { status } = await api.post(`/orgs/${ctx.orgId}/members`, {
      user_id: ctx.userId,
    })
    expect(status).toBe(409)
  })

  it('lists members and shows the added user', async () => {
    const { status, body } = await api.get<Array<{ user_id: string }>>(`/orgs/${ctx.orgId}/members`)
    expect(status).toBe(200)
    expect(body.some((m) => m.user_id === ctx.userId)).toBe(true)
  })

  it('returns 404 when adding a user that does not exist in this app', async () => {
    const { status } = await api.post(`/orgs/${ctx.orgId}/members`, {
      user_id: 'usr_00000000000000000000000000',
    })
    expect(status).toBe(404)
  })

  it('returns 404 when targeting an org from a different app', async () => {
    const { status } = await api.get(`/orgs/org_00000000000000000000000000/members`)
    expect(status).toBe(404)
  })

  it('removes the user from the team org', async () => {
    const { status } = await api.delete(`/orgs/${ctx.orgId}/members/${ctx.userId}`)
    expect(status).toBe(200)

    const { body } = await api.get<Array<{ user_id: string }>>(`/orgs/${ctx.orgId}/members`)
    expect(body.some((m) => m.user_id === ctx.userId)).toBe(false)
  })
})
