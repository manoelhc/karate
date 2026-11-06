import { describe, expect, it, vi } from 'vitest';
import {
  ApiError,
  deletionValue,
  changeDeletion,
  errorKey,
  getSession,
  getTenant,
  login,
  logout,
  request,
  sessionValue,
  tenantValue,
  updateTenant,
  workspaceId,
} from '../src/api';
export const id = '00000000-0000-4000-8000-000000000001';
export const session = {
  tenant_id: id,
  session_id: id,
  expires_at: 2000000000,
};
export const tenant = {
  tenant_id: id,
  name: 'Garden team',
  revision: 1,
  state: 'active' as const,
  can_manage: true,
  deletion: null,
};
const json = (value: unknown, status = 200) =>
  new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  });
function mock(value: unknown, status = 200) {
  return vi.spyOn(globalThis, 'fetch').mockResolvedValue(json(value, status));
}
describe('bounded same-origin API', () => {
  it('normalizes IDs, validates session and tenant payloads', () => {
    expect(workspaceId(` ${id.toUpperCase()} `)).toBe(id);
    expect(workspaceId('no')).toBeNull();
    expect(sessionValue(session)).toEqual(session);
    expect(tenantValue(tenant)).toEqual(tenant);
    for (const v of [
      null,
      {},
      { ...session, tenant_id: 0 },
      { ...session, tenant_id: 'bad' },
      { ...session, session_id: 0 },
      { ...session, session_id: 'bad' },
      { ...session, expires_at: '1' },
      { ...session, expires_at: 1.2 },
      { ...session, expires_at: 0 },
      { ...session, expires_at: 8640000000001 },
    ])
      expect(() => sessionValue(v)).toThrow(ApiError);
    for (const v of [
      null,
      {},
      { ...tenant, tenant_id: 0 },
      { ...tenant, tenant_id: 'bad' },
      { ...tenant, name: 1 },
      { ...tenant, name: ' ' },
      { ...tenant, name: 'a'.repeat(121) },
      { ...tenant, state: 'bad' },
      { ...tenant, revision: '1' },
      { ...tenant, revision: 1.2 },
      { ...tenant, revision: 0 },
      { ...tenant, can_manage: 'yes' },
    ])
      expect(() => tenantValue(v)).toThrow(ApiError);
    expect(tenantValue({ ...tenant, state: 'suspended' }).state).toBe(
      'suspended',
    );
  });
  it('adds only cookie/CSRF protections and bounded signals', async () => {
    const spy = mock(tenant);
    expect(await getTenant(id, new AbortController().signal)).toEqual(tenant);
    expect(spy).toHaveBeenCalledWith(
      `/v1/tenants/${id}`,
      expect.objectContaining({
        credentials: 'same-origin',
        cache: 'no-store',
        redirect: 'error',
        method: 'GET',
      }),
    );
    spy.mockResolvedValueOnce(json(tenant));
    await updateTenant(tenant, 'Renamed', 'suspended');
    expect(spy).toHaveBeenLastCalledWith(
      `/v1/tenants/${id}`,
      expect.objectContaining({
        method: 'PATCH',
        headers: expect.objectContaining({ 'X-Karate-CSRF': '1' }),
        body: JSON.stringify({
          name: 'Renamed',
          state: 'suspended',
          expected_revision: 1,
        }),
      }),
    );
    spy.mockResolvedValue(new Response(null, { status: 204 }));
    await logout(false);
    await logout(true);
    expect(spy).toHaveBeenLastCalledWith(
      '/v1/session/revoke-all',
      expect.objectContaining({ method: 'POST' }),
    );
  });
  it('maps signed out and rejects cross-tenant responses', async () => {
    const spy = mock({}, 401);
    expect(await getSession(new AbortController().signal)).toBeNull();
    spy.mockResolvedValueOnce(json(session));
    expect(await getSession(new AbortController().signal)).toEqual(session);
    spy.mockResolvedValueOnce(json({}, 503));
    await expect(getSession(new AbortController().signal)).rejects.toThrow(
      ApiError,
    );
    spy.mockResolvedValue(
      json({ ...tenant, tenant_id: '00000000-0000-4000-8000-000000000002' }),
    );
    await expect(getTenant(id)).rejects.toThrow(ApiError);
    await expect(updateTenant(tenant, 'Name', 'active')).rejects.toThrow(
      ApiError,
    );
  });
  it('checks trusted backend navigation shape without accepting script URLs', async () => {
    const spy = mock({
      authorization_url: 'https://idp.example/auth?state=opaque',
    });
    expect(await login(id)).toBe('https://idp.example/auth?state=opaque');
    for (const v of [
      null,
      {},
      { authorization_url: 2 },
      { authorization_url: 'a'.repeat(4097) },
      { authorization_url: 'bad' },
      { authorization_url: 'javascript:alert(1)' },
      { authorization_url: 'https://u@idp.example' },
      { authorization_url: 'https://:p@idp.example' },
      { authorization_url: 'https://idp.example/#x' },
    ]) {
      spy.mockResolvedValueOnce(json(v));
      await expect(login(id)).rejects.toThrow(ApiError);
    }
  });
  it('bounds bytes, malformed/absent bodies, timeouts and error codes', async () => {
    const spy = mock({ code: 'VALIDATION' }, 400);
    await expect(request('/test')).rejects.toMatchObject({
      status: 400,
      code: 'VALIDATION',
    });
    spy.mockResolvedValueOnce(json(null, 503));
    await expect(request('/test')).rejects.toMatchObject({ status: 503 });
    for (const response of [
      new Response('{}'),
      new Response(null, { headers: { 'content-type': 'application/json' } }),
      new Response('broken', {
        headers: { 'content-type': 'application/json' },
      }),
      json('a'.repeat(65536)),
    ]) {
      spy.mockResolvedValueOnce(response);
      await expect(request('/test')).rejects.toThrow(ApiError);
    }
    spy.mockRejectedValueOnce(new DOMException('timeout', 'TimeoutError'));
    await expect(request('/test')).rejects.toMatchObject({ status: 0 });
    for (const [status, key] of [
      [401, 'expired'],
      [403, 'forbidden'],
      [409, 'conflict'],
      [429, 'limited'],
      [400, 'invalid'],
      [502, 'unavailable'],
    ] as const)
      expect(errorKey(new ApiError(status))).toBe(key);
    expect(errorKey(null)).toBe('unavailable');
  });
});

it('validates deletion status and sends revisioned confirmation requests', async () => {
  const job = {
    request_id: id,
    state: 'pending',
    requested_at: 1800000000,
    erase_after: 1800604800,
    retention_days: 7,
  };
  expect(deletionValue(null)).toBeNull();
  expect(deletionValue(job)).toEqual(job);
  for (const value of [
    undefined,
    {},
    { ...job, request_id: 1 },
    { ...job, request_id: 'bad' },
    { ...job, state: 'other' },
    { ...job, requested_at: 'x' },
    { ...job, requested_at: 1.5 },
    { ...job, requested_at: -1 },
    { ...job, erase_after: 'x' },
    { ...job, erase_after: 1.5 },
    { ...job, erase_after: -1 },
    { ...job, erase_after: 8640000000001 },
    { ...job, retention_days: '7' },
    { ...job, retention_days: 7.5 },
    { ...job, retention_days: 6 },
    { ...job, retention_days: 31 },
  ])
    expect(() => deletionValue(value)).toThrow(ApiError);
  const spy = mock({ ...tenant, deletion: job });
  await changeDeletion(tenant, id, 7, false);
  expect(spy).toHaveBeenLastCalledWith(
    `/v1/tenants/${id}/deletion`,
    expect.objectContaining({
      body: JSON.stringify({
        confirmation: id,
        retention_days: 7,
        expected_revision: 1,
      }),
    }),
  );
  spy.mockResolvedValue(json({ ...tenant, deletion: job }));
  await changeDeletion(tenant, id, 7, true);
  expect(spy).toHaveBeenLastCalledWith(
    `/v1/tenants/${id}/deletion/cancel`,
    expect.objectContaining({ method: 'POST' }),
  );
  spy.mockResolvedValue(
    json({ ...tenant, tenant_id: '00000000-0000-4000-8000-000000000002' }),
  );
  await expect(changeDeletion(tenant, id, 7, false)).rejects.toThrow(ApiError);
});
