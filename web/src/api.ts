export interface Session {
  tenant_id: string;
  session_id: string;
  expires_at: number;
}
export interface Tenant {
  tenant_id: string;
  name: string;
  state: 'active' | 'suspended';
  revision: number;
  can_manage: boolean;
}
export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string = 'UNAVAILABLE',
  ) {
    super(code);
  }
}
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export const workspaceId = (s: string) =>
  uuid.test(s.trim()) ? s.trim().toLowerCase() : null;
const object = (v: unknown): v is Record<string, unknown> =>
  v !== null && typeof v === 'object';
export function sessionValue(v: unknown): Session {
  if (
    !object(v) ||
    typeof v.tenant_id !== 'string' ||
    !uuid.test(v.tenant_id) ||
    typeof v.session_id !== 'string' ||
    !uuid.test(v.session_id) ||
    typeof v.expires_at !== 'number' ||
    !Number.isSafeInteger(v.expires_at) ||
    v.expires_at <= 0 ||
    v.expires_at > 8640000000000
  )
    throw new ApiError(502);
  return {
    tenant_id: v.tenant_id,
    session_id: v.session_id,
    expires_at: v.expires_at,
  };
}
export function tenantValue(v: unknown): Tenant {
  if (
    !object(v) ||
    typeof v.tenant_id !== 'string' ||
    !uuid.test(v.tenant_id) ||
    typeof v.name !== 'string' ||
    !v.name.trim() ||
    Array.from(v.name).length > 120 ||
    (v.state !== 'active' && v.state !== 'suspended') ||
    typeof v.revision !== 'number' ||
    !Number.isSafeInteger(v.revision) ||
    v.revision < 1 ||
    typeof v.can_manage !== 'boolean'
  )
    throw new ApiError(502);
  return {
    tenant_id: v.tenant_id,
    name: v.name,
    state: v.state,
    revision: v.revision,
    can_manage: v.can_manage,
  };
}
async function boundedJson(response: Response): Promise<unknown> {
  if (
    !response.headers.get('content-type')?.includes('application/json') ||
    !response.body
  )
    throw new ApiError(502);
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const part = await reader.read();
      if (part.done) break;
      size += part.value.length;
      if (size > 65536) throw new ApiError(502);
      chunks.push(part.value);
    }
  } finally {
    await reader.cancel();
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return JSON.parse(
    new TextDecoder('utf-8', { fatal: true }).decode(bytes),
  ) as unknown;
}
export async function request(
  path: string,
  signal?: AbortSignal,
  body?: unknown,
  method = 'POST',
): Promise<unknown> {
  try {
    const response = await fetch(path, {
      method: body === undefined ? 'GET' : method,
      credentials: 'same-origin',
      cache: 'no-store',
      redirect: 'error',
      signal: signal
        ? AbortSignal.any([signal, AbortSignal.timeout(15000)])
        : AbortSignal.timeout(15000),
      headers: {
        Accept: 'application/json',
        ...(body === undefined
          ? {}
          : { 'Content-Type': 'application/json', 'X-Karate-CSRF': '1' }),
      },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });
    if (response.status === 204) return null;
    const value = await boundedJson(response);
    if (!response.ok)
      throw new ApiError(
        response.status,
        object(value) && typeof value.code === 'string'
          ? value.code
          : 'UNAVAILABLE',
      );
    return value;
  } catch (error) {
    if (error instanceof ApiError) throw error;
    throw new ApiError(0);
  }
}
export async function getSession(signal: AbortSignal): Promise<Session | null> {
  try {
    return sessionValue(await request('/v1/session', signal));
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) return null;
    throw error;
  }
}
export async function getTenant(
  id: string,
  signal?: AbortSignal,
): Promise<Tenant> {
  const value = tenantValue(await request(`/v1/tenants/${id}`, signal));
  if (value.tenant_id !== id) throw new ApiError(502);
  return value;
}
export async function login(id: string): Promise<string> {
  const value = await request(`/v1/tenants/${id}/login`, undefined, {});
  if (
    !object(value) ||
    typeof value.authorization_url !== 'string' ||
    value.authorization_url.length > 4096
  )
    throw new ApiError(502);
  let url: URL;
  try {
    url = new URL(value.authorization_url);
  } catch {
    throw new ApiError(502);
  }
  if (url.protocol !== 'https:' || url.username || url.password || url.hash)
    throw new ApiError(502);
  return url.href;
}
export async function updateTenant(
  tenant: Tenant,
  name: string,
  state: Tenant['state'],
): Promise<Tenant> {
  const value = tenantValue(
    await request(
      `/v1/tenants/${tenant.tenant_id}`,
      undefined,
      { name, state, expected_revision: tenant.revision },
      'PATCH',
    ),
  );
  if (value.tenant_id !== tenant.tenant_id) throw new ApiError(502);
  return value;
}
export const logout = (all: boolean) =>
  request(all ? '/v1/session/revoke-all' : '/v1/session/logout', undefined, {});
export function errorKey(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.status === 401) return 'expired';
    if (error.status === 403) return 'forbidden';
    if (error.status === 409) return 'conflict';
    if (error.status === 429) return 'limited';
    if (error.status === 400) return 'invalid';
  }
  return 'unavailable';
}
