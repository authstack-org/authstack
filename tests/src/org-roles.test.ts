import { api } from './helpers/client'
import { ctx } from './helpers/seed'

describe('Organization roles', () => {
  it('lists default roles seeded on org create', async () => {
    const { status, body } = await api.get<
      Array<{ slug: string; name: string; permission_ids: string[] }>
    >(`/orgs/${ctx.orgId}/roles`)

    expect(status).toBe(200)
    expect(body.some((r) => r.slug === 'owner')).toBe(true)
    expect(body.some((r) => r.slug === 'member')).toBe(true)

    const owner = body.find((r) => r.slug === 'owner')
    expect(owner?.permission_ids).toContain(ctx.permissionId)
    ctx.memberRoleId = body.find((r) => r.slug === 'member')?.id
  })

  it('creates a custom organization role with permissions', async () => {
    const { status, body } = await api.post<{
      id: string
      slug: string
      permission_ids: string[]
    }>(`/orgs/${ctx.orgId}/roles`, {
      slug: 'billing-admin',
      name: 'Billing Admin',
      permission_ids: [ctx.permissionId],
    })

    expect(status).toBe(200)
    expect(body.slug).toBe('billing-admin')
    expect(body.permission_ids).toContain(ctx.permissionId)
    ctx.customRoleId = body.id
  })

  it('updates an organization role', async () => {
    const { status, body } = await api.patch<{
      name: string
      permission_ids: string[]
    }>(`/orgs/${ctx.orgId}/roles/${ctx.customRoleId}`, {
      name: 'Billing Administrator',
      permission_ids: [],
    })

    expect(status).toBe(200)
    expect(body.name).toBe('Billing Administrator')
    expect(body.permission_ids).toHaveLength(0)
  })
})
