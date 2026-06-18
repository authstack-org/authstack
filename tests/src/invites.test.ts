import { api } from './helpers/client'
import { ctx } from './helpers/seed'

const BASE_URL = process.env.API_URL ?? 'http://localhost:8080'
const INVITE_EMAIL = `invite-${Date.now()}@example.com`
const INVITE_PASSWORD = 'invitepass123'
const INVITE_NAME = 'Invited User'

let inviteToken = ''
let inviteUrl = ''
let invitedUserId = ''

describe('Invites', () => {
  it('rejects invite creation for a personal organization', async () => {
    const { status: listStatus, body: orgs } = await api.get<
      Array<{ id: string; org_type: string }>
    >('/orgs')
    expect(listStatus).toBe(200)
    const personal = orgs.find((o) => o.org_type === 'personal')
    expect(personal).toBeTruthy()

    const { status } = await api.post(`/orgs/${personal!.id}/invites`, {
      email: `personal-invite-${Date.now()}@example.com`,
      role: 'member',
    })
    expect(status).toBe(422)
  })

  it('creates an invite and returns an invite URL', async () => {
    const { status, body } = await api.post<{
      id: string
      token: string
      invite_url: string
      email: string
      organization_id: string
      role: string
    }>(`/orgs/${ctx.orgId}/invites`, {
      email: INVITE_EMAIL,
      role: 'member',
      name: INVITE_NAME,
    })

    expect(status).toBe(200)
    expect(body.email).toBe(INVITE_EMAIL)
    expect(body.organization_id).toBe(ctx.orgId)
    expect(body.invite_url).toContain('/invite/')
    expect(body.token).toBeTruthy()

    inviteToken = body.token
    inviteUrl = body.invite_url
  })

  it('lists pending invites for the organization', async () => {
    const { status, body } = await api.get<Array<{ email: string; invite_url: string }>>(
      `/orgs/${ctx.orgId}/invites`,
    )
    expect(status).toBe(200)
    expect(body.some((inv) => inv.email === INVITE_EMAIL)).toBe(true)
  })

  it('renders the public accept invite page', async () => {
    const res = await fetch(`${BASE_URL}/invite/${inviteToken}`)
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toContain(INVITE_EMAIL)
    expect(html).toContain('Accept invite')
  })

  it('accepts the invite via JSON API and creates the user', async () => {
    const res = await fetch(`${BASE_URL}/invites/${inviteToken}/accept`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: INVITE_NAME, password: INVITE_PASSWORD }),
    })
    expect(res.status).toBe(200)
    const body = (await res.json()) as { id: string; email: string; name: string }
    expect(body.email).toBe(INVITE_EMAIL)
    expect(body.name).toBe(INVITE_NAME)
    expect(body.id).toBeTruthy()
    invitedUserId = body.id
  })

  it('adds the invited user as a member of the team org', async () => {
    const { status, body } = await api.get<Array<{ user_id: string }>>(`/orgs/${ctx.orgId}/members`)
    expect(status).toBe(200)
    expect(body.some((m) => m.user_id === invitedUserId)).toBe(true)
  })

  it('returns conflict when accepting the same invite again', async () => {
    const res = await fetch(`${BASE_URL}/invites/${inviteToken}/accept`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: INVITE_NAME, password: INVITE_PASSWORD }),
    })
    expect(res.status).toBe(409)
  })

  it('invite URL is stable after listing', async () => {
    expect(inviteUrl).toContain(inviteToken)
  })
})
