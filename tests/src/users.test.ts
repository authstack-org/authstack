import { api } from './helpers/client'
import { ctx } from './helpers/seed'

describe('Users', () => {
  it('lists users and contains the signed-up user', async () => {
    const { status, body } = await api.get<Array<{ id: string; email: string }>>('/users')
    expect(status).toBe(200)
    expect(Array.isArray(body)).toBe(true)
    expect(body.some((u) => u.id === ctx.userId)).toBe(true)
  })

  it('gets a user by id', async () => {
    const { status, body } = await api.get<{
      id: string
      email: string
      name: string
      email_verified: boolean
    }>(`/users/${ctx.userId}`)

    expect(status).toBe(200)
    expect(body.id).toBe(ctx.userId)
    expect(body.email).toBeTruthy()
    expect(body.email_verified).toBe(false)
  })

  it('returns 404 for a non-existent user id', async () => {
    const { status } = await api.get(`/users/usr_00000000000000000000000000`)
    expect(status).toBe(404)
  })

  it('returns 401 when no credentials are provided', async () => {
    const res = await fetch(
      `${process.env.API_URL ?? 'http://localhost:8080'}/users`,
    )
    expect(res.status).toBe(401)
  })
})
