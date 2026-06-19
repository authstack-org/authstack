import { api } from './helpers/client'
import { ctx } from './helpers/seed'

const TEST_EMAIL    = `test-${Date.now()}@example.com`
const TEST_PASSWORD = 'password123'
const TEST_NAME     = 'Test User'

let switchOrgId: string | undefined

function decodeJwtPayload(token: string): Record<string, unknown> {
  const payload = token.split('.')[1]
  return JSON.parse(Buffer.from(payload, 'base64url').toString('utf-8')) as Record<string, unknown>
}

describe('Auth — signup', () => {
  it('returns 422 when email is invalid', async () => {
    const { status } = await api.post('/auth/signup', {
      name: TEST_NAME,
      email: 'not-an-email',
      password: TEST_PASSWORD,
    })
    expect(status).toBe(422)
  })

  it('returns 422 when password is too short', async () => {
    const { status } = await api.post('/auth/signup', {
      name: TEST_NAME,
      email: TEST_EMAIL,
      password: 'short',
    })
    expect(status).toBe(422)
  })

  it('signs up a new user and returns id, email, name', async () => {
    const { status, body } = await api.post<{ id: string; email: string; name: string }>(
      '/auth/signup',
      { name: TEST_NAME, email: TEST_EMAIL, password: TEST_PASSWORD },
    )
    expect(status).toBe(200)
    expect(body.id).toBeTruthy()
    expect(body.email).toBe(TEST_EMAIL)
    expect(body.name).toBe(TEST_NAME)
    ctx.userId = body.id
  })

  it('returns 409 when email is already registered', async () => {
    const { status } = await api.post('/auth/signup', {
      name: TEST_NAME,
      email: TEST_EMAIL,
      password: TEST_PASSWORD,
    })
    expect(status).toBe(409)
  })
})

describe('Auth — login', () => {
  it('returns 401 for wrong password', async () => {
    const { status } = await api.post('/auth/login', {
      email: TEST_EMAIL,
      password: 'wrong-password',
    })
    expect(status).toBe(401)
  })

  it('returns 401 for unknown email', async () => {
    const { status } = await api.post('/auth/login', {
      email: 'nobody@example.com',
      password: TEST_PASSWORD,
    })
    expect(status).toBe(401)
  })

  it('logs in with valid credentials and returns tokens', async () => {
    const { status, body } = await api.post<{
      access_token: string
      refresh_token: string
      token_type: string
    }>('/auth/login', { email: TEST_EMAIL, password: TEST_PASSWORD })

    expect(status).toBe(200)
    expect(body.access_token).toBeTruthy()
    expect(body.refresh_token).toBeTruthy()
    expect(body.token_type).toBe('Bearer')

    const claims = decodeJwtPayload(body.access_token)
    expect(claims.org_id).toBeUndefined()

    ctx.accessToken  = body.access_token
    ctx.refreshToken = body.refresh_token
  })
})

describe('Auth — user organizations and org switching', () => {
  it('lists organizations for the authenticated user', async () => {
    const orgRes = await api.post<{
      id: string
    }>('/orgs', { name: 'Switchable Org', slug: `switchable-${Date.now()}` })
    expect(orgRes.status).toBe(200)
    switchOrgId = orgRes.body.id

    const memberRes = await api.post(`/orgs/${switchOrgId}/members`, {
      user_id: ctx.userId,
      role:    'member',
    })
    expect(memberRes.status).toBe(200)

    const { status, body } = await api.get<Array<{
      organization: { id: string; slug: string }
      role: string
    }>>('/me/organizations', { Authorization: `Bearer ${ctx.accessToken}` })

    expect(status).toBe(200)
    expect(body.some((item) => item.organization.id === switchOrgId && item.role === 'member')).toBe(true)
  })

  it('switches the active organization in the issued token', async () => {
    const { status, body } = await api.post<{
      access_token: string
      refresh_token: string
      token_type: string
    }>('/auth/switch-org', { org_id: switchOrgId }, { Authorization: `Bearer ${ctx.accessToken}` })

    expect(status).toBe(200)
    expect(body.access_token).toBeTruthy()
    expect(body.refresh_token).toBeTruthy()
    expect(body.token_type).toBe('Bearer')

    const claims = decodeJwtPayload(body.access_token)
    expect(claims.org_id).toBe(switchOrgId)
    expect(claims.role).toBe('member')
  })

  it('rejects switching to an organization where the user is not a member', async () => {
    const orgRes = await api.post<{ id: string }>(
      '/orgs',
      { name: 'Not A Member Org', slug: `not-member-${Date.now()}` },
    )
    expect(orgRes.status).toBe(200)

    const { status } = await api.post(
      '/auth/switch-org',
      { org_id: orgRes.body.id },
      { Authorization: `Bearer ${ctx.accessToken}` },
    )
    expect(status).toBe(403)
  })
})

describe('Auth — refresh', () => {
  it('returns a token pair for the requested organization when org_id is supplied', async () => {
    const { status, body } = await api.post<{
      access_token: string
      refresh_token: string
    }>('/auth/refresh', { refresh_token: ctx.refreshToken, org_id: switchOrgId })

    expect(status).toBe(200)
    expect(body.access_token).toBeTruthy()
    expect(body.refresh_token).toBeTruthy()

    const claims = decodeJwtPayload(body.access_token)
    expect(claims.org_id).toBe(switchOrgId)
    expect(claims.role).toBe('member')

    ctx.refreshToken = body.refresh_token
  })

  it('returns a new token pair using the refresh token', async () => {
    const { status, body } = await api.post<{
      access_token: string
      refresh_token: string
    }>('/auth/refresh', { refresh_token: ctx.refreshToken })

    expect(status).toBe(200)
    expect(body.access_token).toBeTruthy()
    expect(body.refresh_token).toBeTruthy()

    const claims = decodeJwtPayload(body.access_token)
    expect(claims.org_id).toBe(switchOrgId)

    ctx.refreshToken = body.refresh_token
  })

  it('returns 401 for an already-used refresh token (rotation)', async () => {
    // The token from before the previous test was rotated — should be rejected
    const { status } = await api.post('/auth/refresh', {
      refresh_token: ctx.refreshToken,
    })
    // First use of the new token succeeds; second use of the same token fails
    // Re-use the current token to force the error
    await api.post('/auth/refresh', { refresh_token: ctx.refreshToken })
    const { status: rotatedStatus } = await api.post('/auth/refresh', {
      refresh_token: ctx.refreshToken,
    })
    expect(rotatedStatus).toBe(401)
  })

  it('returns 401 for a completely invalid token', async () => {
    const { status } = await api.post('/auth/refresh', {
      refresh_token: 'not.a.token',
    })
    expect(status).toBe(401)
  })
})

describe('Auth — logout', () => {
  it('logs back in and then logs out', async () => {
    const loginRes = await api.post<{ refresh_token: string }>(
      '/auth/login',
      { email: TEST_EMAIL, password: TEST_PASSWORD },
    )
    expect(loginRes.status).toBe(200)

    const { status } = await api.post('/auth/logout', {
      refresh_token: loginRes.body.refresh_token,
    })
    expect(status).toBe(200)

    // Refresh after logout should fail
    const { status: refreshStatus } = await api.post('/auth/refresh', {
      refresh_token: loginRes.body.refresh_token,
    })
    expect(refreshStatus).toBe(401)
  })
})

describe('Auth — app isolation', () => {
  it('returns 401 when no Authorization header is sent', async () => {
    const res = await fetch(
      `${process.env.API_URL ?? 'http://localhost:8080'}/auth/login`,
      {
        method:  'POST',
        headers: { 'Content-Type': 'application/json' },
        body:    JSON.stringify({ email: TEST_EMAIL, password: TEST_PASSWORD }),
      },
    )
    expect(res.status).toBe(401)
  })

  it('denies login to another app in the same directory without a grant', async () => {
    const baseUrl = process.env.API_URL ?? 'http://localhost:8080'
    const adminEmail = process.env.AUTHSTACK_ADMIN_EMAIL ?? 'test-admin@authstack.local'
    const adminPassword = process.env.AUTHSTACK_ADMIN_PASSWORD ?? 'test-admin-password-123'

    const loginRes = await fetch(`${baseUrl}/admin/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ email: adminEmail, password: adminPassword }).toString(),
      redirect: 'manual',
    })
    expect(loginRes.status).toBe(303)
    const adminCookie = loginRes.headers.get('set-cookie')!.split(';')[0]

    const appRes = await fetch(`${baseUrl}/admin/applications`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Cookie: adminCookie,
      },
      body: JSON.stringify({ name: `other-app-${Date.now()}` }),
    })
    expect(appRes.ok).toBe(true)
    const otherApp = await appRes.json() as { id: string; client_secret: string }

    const auth = 'Basic ' + Buffer.from(`${otherApp.id}:${otherApp.client_secret}`).toString('base64')
    const res = await fetch(`${baseUrl}/auth/login`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: auth,
      },
      body: JSON.stringify({ email: TEST_EMAIL, password: TEST_PASSWORD }),
    })
    expect(res.status).toBe(401)
  })

  it('returns 409 when signing up on another app in the same directory with an existing email', async () => {
    const baseUrl = process.env.API_URL ?? 'http://localhost:8080'
    const adminEmail = process.env.AUTHSTACK_ADMIN_EMAIL ?? 'test-admin@authstack.local'
    const adminPassword = process.env.AUTHSTACK_ADMIN_PASSWORD ?? 'test-admin-password-123'

    const loginRes = await fetch(`${baseUrl}/admin/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ email: adminEmail, password: adminPassword }).toString(),
      redirect: 'manual',
    })
    const adminCookie = loginRes.headers.get('set-cookie')!.split(';')[0]

    const appRes = await fetch(`${baseUrl}/admin/applications`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Cookie: adminCookie,
      },
      body: JSON.stringify({ name: `signup-block-${Date.now()}` }),
    })
    const otherApp = await appRes.json() as { id: string; client_secret: string }
    const auth = 'Basic ' + Buffer.from(`${otherApp.id}:${otherApp.client_secret}`).toString('base64')

    const res = await fetch(`${baseUrl}/auth/signup`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: auth,
      },
      body: JSON.stringify({
        name: 'Duplicate User',
        email: TEST_EMAIL,
        password: TEST_PASSWORD,
      }),
    })
    expect(res.status).toBe(409)
  })
})
