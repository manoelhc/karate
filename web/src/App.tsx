import { useEffect, useRef, useState, type FormEvent } from 'react';
import {
  QueryClient,
  QueryClientProvider,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  ApiError,
  errorKey,
  getSession,
  getTenant,
  login,
  logout,
  updateTenant,
  workspaceId,
  type Session,
  type Tenant,
} from './api';

export function newClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 60000, refetchOnWindowFocus: true },
      mutations: { retry: false, gcTime: 60000 },
    },
  });
}
export function App({
  client,
  navigate = (url) => window.location.assign(url),
}: {
  client: QueryClient;
  navigate?: (url: string) => void;
}) {
  return (
    <QueryClientProvider client={client}>
      <Workspace navigate={navigate} />
    </QueryClientProvider>
  );
}
function Notice({
  message,
  alert = false,
}: {
  message: string;
  alert?: boolean;
}) {
  return (
    <p
      className={alert ? 'notice error' : 'notice'}
      role={alert ? 'alert' : 'status'}
    >
      {message}
    </p>
  );
}
function Workspace({ navigate }: { navigate: (url: string) => void }) {
  const { t, i18n } = useTranslation();
  const client = useQueryClient();
  const [notice, setNotice] = useState(() =>
    new URLSearchParams(window.location.search).get('signin') === 'failed'
      ? 'signinFailed'
      : '',
  );
  const session = useQuery({
    queryKey: ['session'],
    queryFn: ({ signal }) => getSession(signal),
  });
  const endSession = (reason: string) => {
    void client.cancelQueries();
    client.removeQueries({ queryKey: ['tenant'] });
    client.getMutationCache().clear();
    client.setQueryData(['session'], null);
    setNotice(reason);
  };
  useEffect(() => {
    if (!session.isPending && !session.data) {
      client.removeQueries({ queryKey: ['tenant'] });
      client.getMutationCache().clear();
    }
  }, [session.isPending, session.data, client]);
  // Absolute expiry does not keep idle sessions alive. Server remains authoritative.
  useEffect(() => {
    if (!session.data) return;
    const timer = window.setTimeout(
      () => {
        void client.cancelQueries();
        client.removeQueries({ queryKey: ['tenant'] });
        client.getMutationCache().clear();
        client.setQueryData(['session'], null);
        setNotice('expired');
      },
      Math.min(
        Math.max(0, session.data.expires_at * 1000 - Date.now()),
        2147483647,
      ),
    );
    return () => window.clearTimeout(timer);
  }, [session.data, client]);
  useEffect(() => {
    if (new URLSearchParams(window.location.search).has('signin'))
      window.history.replaceState(null, '', '/');
  }, []);
  return (
    <>
      <a className="skip" href="#main">
        {t('skip')}
      </a>
      <header className="topbar">
        <a className="brand" href="/" aria-label={t('brand')}>
          <span className="brand-mark" aria-hidden="true" />
          {t('brand')}
        </a>
        <label className="locale">
          {t('language')}
          <select
            value={i18n.language}
            onChange={(e) => {
              void i18n.changeLanguage(e.target.value);
            }}
          >
            <option value="en">{t('english')}</option>
            <option value="pt-BR">{t('portuguese')}</option>
          </select>
        </label>
      </header>
      <main id="main" tabIndex={-1}>
        {session.isPending ? (
          <div className="loading" role="status">
            <span className="skeleton" />
            {t('loading')}
          </div>
        ) : session.isError &&
          session.error instanceof ApiError &&
          session.error.status === 403 ? (
          <SignIn navigate={navigate} notice="forbidden" />
        ) : session.isError ? (
          <section className="panel recovery">
            <h1>{t('signinTitle')}</h1>
            <Notice message={t(errorKey(session.error))} alert />
            <button
              onClick={() => {
                void session.refetch();
              }}
            >
              {t('retry')}
            </button>
          </section>
        ) : session.data ? (
          <SignedIn
            key={session.data.session_id}
            session={session.data}
            endSession={endSession}
          />
        ) : (
          <SignIn navigate={navigate} notice={notice} />
        )}
      </main>
      <footer>{t('footer')}</footer>
    </>
  );
}
function SignIn({
  navigate,
  notice,
}: {
  navigate: (url: string) => void;
  notice: string;
}) {
  const { t } = useTranslation();
  const [id, setId] = useState('');
  const [invalid, setInvalid] = useState(false);
  const signin = useMutation({
    mutationFn: (id: string) => login(id),
    onSuccess: (url) => navigate(url),
  });
  const submit = (e: FormEvent) => {
    e.preventDefault();
    const value = workspaceId(id);
    setInvalid(!value);
    if (value) signin.mutate(value);
  };
  return (
    <div className="entry">
      <section className="welcome">
        <p className="eyebrow">{t('eyebrow')}</p>
        <h1>{t('hero')}</h1>
        <p>{t('intro')}</p>
        <div className="arc" aria-hidden="true" />
        <p className="welcome-note">{t('note')}</p>
      </section>
      <section className="panel signin">
        <span className="section-number" aria-hidden="true">
          {'01 /'}
        </span>
        <h2>{t('signinTitle')}</h2>
        <p className="muted">{t('signinIntro')}</p>
        {notice && (
          <Notice message={t(notice)} alert={notice !== 'signedOut'} />
        )}
        <form onSubmit={submit} noValidate>
          <label htmlFor="workspace-id">{t('workspaceId')}</label>
          <input
            id="workspace-id"
            value={id}
            onChange={(e) => {
              setId(e.target.value);
              setInvalid(false);
            }}
            autoComplete="off"
            spellCheck={false}
            maxLength={64}
            aria-describedby={invalid ? 'id-error' : 'id-hint'}
            aria-invalid={invalid}
            disabled={signin.isPending}
          />
          <p className="hint" id="id-hint">
            {t('idHint')}
          </p>
          {invalid && (
            <p role="alert" id="id-error">
              {t('invalidId')}
            </p>
          )}
          {signin.isError && (
            <Notice message={t(errorKey(signin.error))} alert />
          )}
          <button className="primary full" disabled={signin.isPending}>
            {t(signin.isPending ? 'connecting' : 'continue')}
          </button>
        </form>
      </section>
    </div>
  );
}
function SignedIn({
  session,
  endSession,
}: {
  session: Session;
  endSession: (reason: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const client = useQueryClient();
  const [updated, setUpdated] = useState(false);
  const isCurrent = () =>
    client.getQueryData<Session | null>(['session'])?.session_id ===
    session.session_id;
  const tenant = useQuery({
    queryKey: ['tenant', session.tenant_id],
    queryFn: ({ signal }) => getTenant(session.tenant_id, signal),
  });
  const leave = useMutation({
    mutationFn: (all: boolean) => logout(all),
    onSuccess: () => {
      if (isCurrent()) endSession('signedOut');
    },
    onError: (error) => {
      if (isCurrent() && error instanceof ApiError && error.status === 401)
        endSession('expired');
    },
  });
  useEffect(() => {
    if (tenant.error instanceof ApiError && tenant.error.status === 401) {
      client.removeQueries({ queryKey: ['tenant'] });
      client.getMutationCache().clear();
      client.setQueryData(['session'], null);
    }
  }, [tenant.error, client]);
  return (
    <div className="workspace">
      <div className="workspace-heading">
        <div>
          <p className="eyebrow">{t('workspace')}</p>
          <h1>{t('overview')}</h1>
        </div>
        <button onClick={() => leave.mutate(false)} disabled={leave.isPending}>
          {t(leave.isPending ? 'signingOut' : 'signout')}
        </button>
      </div>
      {updated && <Notice message={t('saved')} />}
      {leave.isError && <Notice message={t(errorKey(leave.error))} alert />}
      {tenant.isPending ? (
        <div role="status" className="panel">
          <span className="skeleton" />
          {t('loading')}
        </div>
      ) : tenant.isError ? (
        <section className="panel">
          <Notice message={t(errorKey(tenant.error))} alert />
          <button
            onClick={() => {
              void tenant.refetch();
            }}
          >
            {t('retry')}
          </button>
        </section>
      ) : (
        <TenantPanel
          key={`${tenant.data.revision}-${tenant.data.can_manage}`}
          tenant={tenant.data}
          sessionId={session.session_id}
          onChanged={() => setUpdated(true)}
          refresh={() => tenant.refetch().then(() => undefined)}
          endSession={endSession}
        />
      )}
      <section className="session">
        <div>
          <h2>{t('session')}</h2>
          <p>{t('sessionHint')}</p>
          <p className="hint">
            {t('expires', {
              time: new Intl.DateTimeFormat(i18n.language, {
                dateStyle: 'medium',
                timeStyle: 'short',
              }).format(new Date(session.expires_at * 1000)),
            })}
          </p>
          <p className="hint">{t('signoutNote')}</p>
        </div>
        <button disabled={leave.isPending} onClick={() => leave.mutate(true)}>
          {t('signoutAll')}
        </button>
      </section>
    </div>
  );
}
function TenantPanel({
  tenant,
  sessionId,
  onChanged,
  refresh,
  endSession,
}: {
  tenant: Tenant;
  sessionId: string;
  onChanged: () => void;
  refresh: () => Promise<void>;
  endSession: (reason: string) => void;
}) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const isCurrent = () =>
    client.getQueryData<Session | null>(['session'])?.session_id === sessionId;
  const [name, setName] = useState(tenant.name);
  const [refreshing, setRefreshing] = useState(false);
  const [blocked, setBlocked] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const change = useMutation({
    mutationFn: ({ name, state }: { name: string; state: Tenant['state'] }) =>
      updateTenant(tenant, name, state),
    onSuccess: (value) => {
      if (isCurrent()) {
        client.setQueryData(['tenant', tenant.tenant_id], value);
        onChanged();
      }
    },
    onError: (error) => {
      if (isCurrent() && error instanceof ApiError && error.status === 401)
        endSession('expired');
      else setBlocked(true);
    },
  });
  const close = () => {
    dialog.current?.close();
    trigger.current?.focus();
  };
  const submit = (e: FormEvent) => {
    e.preventDefault();
    change.mutate({ name: name.trim(), state: tenant.state });
  };
  const suspended = tenant.state === 'suspended';
  return (
    <>
      <section className="panel tenant-panel">
        <div className="tenant-heading">
          <div>
            <h2>{tenant.name}</h2>
            <p className="id-value">{tenant.tenant_id}</p>
          </div>
          <span className={`badge ${tenant.state}`}>{t(tenant.state)}</span>
        </div>
        {suspended && <Notice message={t('suspendedNote')} />}
        {change.isError && <Notice message={t(errorKey(change.error))} alert />}
        {blocked && (
          <div className="notice">
            <p>{t('refreshRequired')}</p>
            <button
              disabled={refreshing}
              onClick={async () => {
                setRefreshing(true);
                await refresh();
                setRefreshing(false);
                setBlocked(false);
                change.reset();
              }}
            >
              {t('refresh')}
            </button>
          </div>
        )}
        <div className="settings-grid">
          <section>
            <h3>{t('details')}</h3>
            {tenant.can_manage ? (
              <form onSubmit={submit}>
                <label htmlFor="tenant-name">{t('name')}</label>
                <input
                  id="tenant-name"
                  value={name}
                  maxLength={120}
                  required
                  onChange={(e) => setName(e.target.value)}
                  aria-describedby="name-hint"
                  disabled={change.isPending || blocked}
                />
                <p id="name-hint" className="hint">
                  {t('nameHint')}
                </p>
                <button
                  disabled={
                    !name.trim() ||
                    name.trim() === tenant.name ||
                    change.isPending ||
                    blocked
                  }
                >
                  {t(change.isPending ? 'saving' : 'save')}
                </button>
              </form>
            ) : (
              <p className="muted">{t('readOnly')}</p>
            )}
          </section>
          {tenant.can_manage && (
            <section>
              <h3>{t('lifecycle')}</h3>
              <p className="muted">{t('lifecycleIntro')}</p>
              <button
                ref={trigger}
                className={suspended ? 'primary' : 'danger'}
                disabled={change.isPending || blocked}
                onClick={() => dialog.current?.showModal()}
              >
                {t(suspended ? 'resume' : 'suspend')}
              </button>
            </section>
          )}
        </div>
      </section>
      <dialog
        ref={dialog}
        aria-labelledby="confirm-title"
        aria-describedby="confirm-detail"
        onCancel={() => trigger.current?.focus()}
      >
        <h2 id="confirm-title">
          {t(suspended ? 'confirmResume' : 'confirmSuspend')}
        </h2>
        <p id="confirm-detail">
          {t(suspended ? 'resumeDetail' : 'suspendDetail')}
        </p>
        <div className="dialog-actions">
          <button onClick={close}>{t('cancel')}</button>
          <button
            className={suspended ? 'primary' : 'danger'}
            onClick={() => {
              close();
              change.mutate({
                name: tenant.name,
                state: suspended ? 'active' : 'suspended',
              });
            }}
          >
            {t('confirm')}
          </button>
        </div>
      </dialog>
    </>
  );
}
