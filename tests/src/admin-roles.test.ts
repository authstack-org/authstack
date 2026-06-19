// Directory admin role: scoped access, operator delegation, and edge cases.

import { ctx } from './helpers/ctx'

const BASE_URL       = process.env.API_URL ?? 'http://localhost:8080'
const ADMIN_EMAIL    = process.env.AUTHSTACK_ADMIN_EMAIL ?? 'test-admin@authstack.local'
const ADMIN_PASSWORD = process.env.AUTHSTACK_ADMIN_PASSWORD ?? 'test-admin-password-123'

async function login(email: string, password: string): Promise<string> {
  const res = await fetch(`${BASE_URL}/admin/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({ email, password }).toString(),
    redirect: 'manual',
  })
  expect(res.status).toBe(303)
  const raw = res.headers.get('set-cookie')
  if (!raw) throw new Error('login did not return Set-Cookie')
  return raw.split(';')[0]
}

async function loginInstanceAdmin(): Promise<string> {
  return login(ADMIN_EMAIL, ADMIN_PASSWORD)
}

function getDirectoryIdByName(html: string, name: string): string {
  for (const part of html.split('name="directory_ids"').slice(1)) {
    if (part.includes(name)) {
      const match = part.match(/value="(dir_[^"]+)"/)
      if (match) return match[1]
    }
  }
  throw new Error(`directory id for name ${name} not found`)
}

async function createDirectory(cookie: string, name: string, slug: string): Promise<string> {
  const res = await fetch(`${BASE_URL}/admin/directories/new`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/x-www-form-urlencoded',
      Cookie: cookie,
    },
    body: new URLSearchParams({
      name,
      slug,
    }).toString(),
    redirect: 'manual',
  })
  expect(res.status).toBe(303)

  const form = await fetch(`${BASE_URL}/admin/operators/new`, { headers: { Cookie: cookie } })
  expect(form.status).toBe(200)
  return getDirectoryIdByName(await form.text(), name)
}

async function createAppInDirectory(
  cookie: string,
  name: string,
  directoryId: string,
): Promise<string> {
  const res = await fetch(`${BASE_URL}/admin/apps`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/x-www-form-urlencoded',
      Cookie: cookie,
    },
    body: new URLSearchParams({ name, directory_id: directoryId }).toString(),
    redirect: 'manual',
  })
  expect(res.status).toBe(200)
  const html = await res.text()
  const match = html.match(/Client Secret[\s\S]*?(app_[a-z0-9]+)/)
  if (!match) {
    const fallback = html.match(/app_[a-z0-9]+/g)
    if (!fallback?.length) throw new Error('created app id not found in response')
    return fallback[0]
  }
  return match[1]
}

async function createOperator(
  cookie: string,
  params: {
    email: string
    password: string
    role: string
    app_ids?: string[]
    directory_ids?: string[]
  },
): Promise<void> {
  const body = new URLSearchParams({
    email: params.email,
    password: params.password,
    role: params.role,
  })
  for (const id of params.app_ids ?? []) body.append('app_ids', id)
  for (const id of params.directory_ids ?? []) body.append('directory_ids', id)

  const res = await fetch(`${BASE_URL}/admin/operators/new`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/x-www-form-urlencoded',
      Cookie: cookie,
    },
    body: body.toString(),
    redirect: 'manual',
  })
  expect(res.status).toBe(303)
  expect(res.headers.get('location')).toBe('/admin/operators')
}

describe('Admin — directory admin role', () => {
  const ts = Date.now()
  const dirSlug = `scoped-${ts}`
  const dirAdminEmail = `dir-admin-${ts}@authstack.local`
  const dirAdminPassword = 'dir-admin-password-123'

  let instanceCookie: string
  let directoryId: string
  let scopedAppId: string
  let defaultAppId: string
  let dirAdminCookie: string

  beforeAll(async () => {
    instanceCookie = await loginInstanceAdmin()
    directoryId = await createDirectory(instanceCookie, 'Scoped Directory', dirSlug)
    expect(directoryId).not.toBe('dir_00000000000000000000000001')
    scopedAppId = await createAppInDirectory(instanceCookie, `Scoped App ${ts}`, directoryId)
    defaultAppId = ctx.clientId
    if (!defaultAppId) throw new Error('missing default app in test context')

    await createOperator(instanceCookie, {
      email: dirAdminEmail,
      password: dirAdminPassword,
      role: 'directory_admin',
      directory_ids: [directoryId],
    })

    dirAdminCookie = await login(dirAdminEmail, dirAdminPassword)
  })

  it('sees only applications in granted directories on the dashboard', async () => {
    const res = await fetch(`${BASE_URL}/admin/dashboard`, {
      headers: { Cookie: dirAdminCookie },
    })
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toContain(scopedAppId)
    expect(html).not.toContain(defaultAppId)
    expect(html).toContain('New application')
  })

  it('can view scoped directories list but cannot create directories', async () => {
    const res = await fetch(`${BASE_URL}/admin/directories`, {
      headers: { Cookie: dirAdminCookie },
    })
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toContain('Scoped Directory')
    expect(html).not.toContain('New directory')
    expect(html).not.toContain('default')

    const newRes = await fetch(`${BASE_URL}/admin/directories/new`, {
      headers: { Cookie: dirAdminCookie },
      redirect: 'manual',
    })
    expect(newRes.status).toBe(303)
    expect(newRes.headers.get('location')).toBe('/admin/dashboard')
  })

  it('can open directory detail and add a directory admin from the directory UI', async () => {
    const detailRes = await fetch(`${BASE_URL}/admin/directories/${directoryId}`, {
      headers: { Cookie: dirAdminCookie },
    })
    expect(detailRes.status).toBe(200)
    const detailHtml = await detailRes.text()
    expect(detailHtml).toContain('Add directory admin')
    expect(detailHtml).toContain(dirAdminEmail)

    const email = `ui-dir-admin-${ts}@authstack.local`
    const createRes = await fetch(`${BASE_URL}/admin/directories/${directoryId}/admins/new`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
        Cookie: dirAdminCookie,
      },
      body: new URLSearchParams({
        email,
        password: 'ui-dir-admin-password-123',
      }).toString(),
      redirect: 'manual',
    })
    expect(createRes.status).toBe(303)
    expect(createRes.headers.get('location')).toBe(`/admin/directories/${directoryId}`)

    const afterRes = await fetch(`${BASE_URL}/admin/directories/${directoryId}`, {
      headers: { Cookie: dirAdminCookie },
    })
    expect(await afterRes.text()).toContain(email)

    const cookie = await login(email, 'ui-dir-admin-password-123')
    const dash = await fetch(`${BASE_URL}/admin/dashboard`, { headers: { Cookie: cookie } })
    expect(dash.status).toBe(200)
    expect(await dash.text()).toContain(scopedAppId)
  })

  it('can open scoped app detail but not apps in other directories', async () => {
    const allowed = await fetch(`${BASE_URL}/admin/apps/${scopedAppId}`, {
      headers: { Cookie: dirAdminCookie },
      redirect: 'manual',
    })
    expect(allowed.status).toBe(200)

    const denied = await fetch(`${BASE_URL}/admin/apps/${defaultAppId}`, {
      headers: { Cookie: dirAdminCookie },
      redirect: 'manual',
    })
    expect(denied.status).toBe(303)
    expect(denied.headers.get('location')).toBe('/admin/dashboard')
  })

  it('can create a new application in its directory', async () => {
    const newAppPage = await fetch(`${BASE_URL}/admin/apps/new`, {
      headers: { Cookie: dirAdminCookie },
    })
    expect(newAppPage.status).toBe(200)
    const pageHtml = await newAppPage.text()
    expect(pageHtml).toContain(directoryId)

    const res = await fetch(`${BASE_URL}/admin/apps`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
        Cookie: dirAdminCookie,
      },
      body: new URLSearchParams({
        name: `Dir admin app ${ts}`,
        directory_id: directoryId,
      }).toString(),
      redirect: 'manual',
    })
    expect(res.status).toBe(200)
    expect(await res.text()).toContain('Application created')
  })

  it('cannot create an application in another directory', async () => {
    const res = await fetch(`${BASE_URL}/admin/apps`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
        Cookie: dirAdminCookie,
      },
      body: new URLSearchParams({
        name: 'Should fail',
        directory_id: 'dir_00000000000000000000000001',
      }).toString(),
      redirect: 'manual',
    })
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toContain('You do not have access to that directory')
  })

  it('can manage operators but not see instance admins', async () => {
    const res = await fetch(`${BASE_URL}/admin/operators`, {
      headers: { Cookie: dirAdminCookie },
    })
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toContain('Add operator')
    expect(html).not.toContain(ADMIN_EMAIL)
    expect(html).not.toContain('Instance admin')
  })

  it('can create an app admin for apps in its directory', async () => {
    const email = `nested-app-admin-${ts}@authstack.local`
    await createOperator(dirAdminCookie, {
      email,
      password: 'nested-app-admin-123',
      role: 'app_admin',
      app_ids: [scopedAppId],
    })

    const cookie = await login(email, 'nested-app-admin-123')
    const dash = await fetch(`${BASE_URL}/admin/dashboard`, { headers: { Cookie: cookie } })
    expect(dash.status).toBe(200)
    expect(await dash.text()).toContain(scopedAppId)
  })

  it('rejects creating an app admin for apps outside its directory', async () => {
    const res = await fetch(`${BASE_URL}/admin/operators/new`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
        Cookie: dirAdminCookie,
      },
      body: new URLSearchParams({
        email: `bad-app-admin-${ts}@authstack.local`,
        password: 'bad-app-admin-123',
        role: 'app_admin',
        app_ids: defaultAppId,
      }).toString(),
      redirect: 'manual',
    })
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toMatch(/outside your scope|permission to assign/)
  })

  it('can create a nested directory admin within its directories', async () => {
    const email = `nested-dir-admin-${ts}@authstack.local`
    await createOperator(dirAdminCookie, {
      email,
      password: 'nested-dir-admin-123',
      role: 'directory_admin',
      directory_ids: [directoryId],
    })

    const cookie = await login(email, 'nested-dir-admin-123')
    const dash = await fetch(`${BASE_URL}/admin/dashboard`, { headers: { Cookie: cookie } })
    expect(dash.status).toBe(200)
    expect(await dash.text()).toContain(scopedAppId)
  })

  it('cannot create an instance admin', async () => {
    const res = await fetch(`${BASE_URL}/admin/operators/new`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
        Cookie: dirAdminCookie,
      },
      body: new URLSearchParams({
        email: `bad-instance-${ts}@authstack.local`,
        password: 'bad-instance-123',
        role: 'instance_admin',
      }).toString(),
      redirect: 'manual',
    })
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toMatch(/permission to assign|Invalid operator role/)
  })

  it('cannot grant a directory outside its scope', async () => {
    const res = await fetch(`${BASE_URL}/admin/operators/new`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
        Cookie: dirAdminCookie,
      },
      body: new URLSearchParams({
        email: `bad-dir-grant-${ts}@authstack.local`,
        password: 'bad-dir-grant-123',
        role: 'directory_admin',
        directory_ids: 'dir_00000000000000000000000001',
      }).toString(),
      redirect: 'manual',
    })
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toMatch(/outside your scope|permission to assign/)
  })
})

describe('Admin — app admin operator restrictions', () => {
  it('cannot access the operators page', async () => {
    const instanceCookie = await loginInstanceAdmin()
    const email = `operators-denied-${Date.now()}@authstack.local`
    const password = 'operators-denied-123'
    const appId = ctx.clientId
    if (!appId) throw new Error('missing clientId')

    await createOperator(instanceCookie, {
      email,
      password,
      role: 'app_admin',
      app_ids: [appId],
    })

    const cookie = await login(email, password)
    const res = await fetch(`${BASE_URL}/admin/operators`, {
      headers: { Cookie: cookie },
      redirect: 'manual',
    })
    expect(res.status).toBe(303)
    expect(res.headers.get('location')).toBe('/admin/dashboard')
  })
})
