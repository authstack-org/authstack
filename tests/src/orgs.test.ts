import { api } from './helpers/client'
import { ctx } from './helpers/seed'

const SLUG = `test-org-${Date.now()}`

describe('Organizations', () => {
  it('lists orgs for the application', async () => {
    const { status, body } = await api.get<Array<{ id: string; slug: string }>>('/orgs')
    expect(status).toBe(200)
    expect(Array.isArray(body)).toBe(true)
  })

  it('creates an organization', async () => {
    const { status, body } = await api.post<{
      id: string
      name: string
      slug: string
      application_id: string
    }>('/orgs', { name: 'Test Org', slug: SLUG })

    expect(status).toBe(200)
    expect(body.id).toBeTruthy()
    expect(body.slug).toBe(SLUG)
    expect(body.application_id).toBe(ctx.clientId)

    ctx.orgId = body.id
  })

  it('returns 422 when slug is empty', async () => {
    const { status } = await api.post('/orgs', { name: 'Bad Org', slug: '' })
    expect(status).toBe(422)
  })

  it('returns 409 when slug is already taken within the app', async () => {
    const { status } = await api.post('/orgs', { name: 'Duplicate', slug: SLUG })
    expect(status).toBe(409)
  })

  it('gets the org by id', async () => {
    const { status, body } = await api.get<{ id: string; slug: string }>(`/orgs/${ctx.orgId}`)
    expect(status).toBe(200)
    expect(body.id).toBe(ctx.orgId)
    expect(body.slug).toBe(SLUG)
  })

  it('returns 404 for a non-existent org id', async () => {
    const { status } = await api.get(`/orgs/org_00000000000000000000000000`)
    expect(status).toBe(404)
  })
})
