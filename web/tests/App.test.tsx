import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { App, newClient } from '../src/App';
import * as api from '../src/api';
import i18n, { detectedLocale, localeDocument } from '../src/i18n';
const id = '00000000-0000-4000-8000-000000000001';
const session = {
  tenant_id: id,
  session_id: id,
  expires_at: Math.floor(Date.now() / 1000) + 3600,
};
const tenant = {
  tenant_id: id,
  name: 'Garden team',
  revision: 1,
  state: 'active' as const,
  can_manage: true,
  deletion: null,
};
beforeEach(async () => {
  await i18n.changeLanguage('en');
  window.history.replaceState(null, '', '/');
});
function setup(s: api.Session | null = session, t: api.Tenant = tenant) {
  vi.spyOn(api, 'getSession').mockResolvedValue(s);
  vi.spyOn(api, 'getTenant').mockResolvedValue(t);
  const client = newClient();
  const navigate = vi.fn();
  const view = render(<App client={client} navigate={navigate} />);
  return { client, navigate, ...view };
}
it('loads, validates ID, reports sign-in error, then navigates', async () => {
  const start = vi
    .spyOn(api, 'login')
    .mockRejectedValueOnce(new api.ApiError(503))
    .mockResolvedValue('https://idp.example/auth');
  const { navigate } = setup(null);
  expect(screen.getByRole('status')).toHaveTextContent('Opening');
  await screen.findByLabelText('Workspace ID');
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Continue to sign in' }));
  expect(screen.getByRole('alert')).toHaveTextContent('valid workspace');
  await user.type(screen.getByLabelText('Workspace ID'), id);
  await user.click(screen.getByRole('button', { name: 'Continue to sign in' }));
  await screen.findByText('The workspace is unavailable. Please try again.');
  await user.click(screen.getByRole('button', { name: 'Continue to sign in' }));
  await waitFor(() =>
    expect(navigate).toHaveBeenCalledWith('https://idp.example/auth'),
  );
  expect(start).toHaveBeenCalledTimes(2);
});
it('shows callback failure, cleans its URL and changes locale', async () => {
  window.history.replaceState(null, '', '/?signin=failed');
  setup(null);
  await screen.findByText(/Sign-in could not/);
  expect(location.search).toBe('');
  await userEvent.selectOptions(screen.getByLabelText('Language'), 'pt-BR');
  expect(
    screen.getByRole('heading', { name: 'Acesso ao espaço' }),
  ).toBeInTheDocument();
  expect(document.documentElement.lang).toBe('pt-BR');
});
it('renders session error and retries without displaying stale metadata', async () => {
  const client = newClient();
  vi.spyOn(api, 'getSession')
    .mockRejectedValueOnce(new api.ApiError(503))
    .mockResolvedValue(null);
  render(<App client={client} />);
  await screen.findByRole('alert');
  await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
  await screen.findByLabelText('Workspace ID');
});
it('renames and confirms suspension, cancellation returns focus', async () => {
  vi.spyOn(api, 'updateTenant').mockImplementation(async (t, name, state) => ({
    ...t,
    name,
    state,
    revision: t.revision + 1,
  }));
  setup();
  await screen.findByRole('heading', { name: 'Garden team' });
  const user = userEvent.setup();
  const input = screen.getByLabelText('Workspace name');
  await user.clear(input);
  expect(screen.getByRole('button', { name: 'Save name' })).toBeDisabled();
  await user.type(input, 'Orchard');
  await user.click(screen.getByRole('button', { name: 'Save name' }));
  await screen.findByRole('heading', { name: 'Orchard' });
  const trigger = screen.getByRole('button', { name: 'Suspend workspace' });
  await user.click(trigger);
  expect(screen.getByRole('dialog')).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(trigger).toHaveFocus();
  await user.click(trigger);
  fireEvent(screen.getByRole('dialog'), new Event('cancel'));
  await user.click(screen.getByRole('button', { name: 'Confirm change' }));
  await screen.findByText('Suspended');
  await user.click(screen.getByRole('button', { name: 'Resume workspace' }));
  expect(
    screen.getByRole('heading', { name: 'Resume this workspace?' }),
  ).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Confirm change' }));
  await screen.findByText('Active');
});
it('requires a fresh read after ambiguous changes and handles read failures', async () => {
  vi.spyOn(api, 'updateTenant').mockRejectedValue(new api.ApiError(409));
  setup();
  await screen.findByLabelText('Workspace name');
  await userEvent.clear(screen.getByLabelText('Workspace name'));
  await userEvent.type(screen.getByLabelText('Workspace name'), 'Changed');
  await userEvent.click(screen.getByRole('button', { name: 'Save name' }));
  await screen.findByText(/workspace changed/);
  expect(
    screen.getByRole('button', { name: 'Suspend workspace' }),
  ).toBeDisabled();
  await userEvent.click(
    screen.getByRole('button', { name: 'Refresh workspace' }),
  );
  await waitFor(() =>
    expect(
      screen.getByRole('button', { name: 'Suspend workspace' }),
    ).toBeEnabled(),
  );
});
it('read-only members have no controls; logout errors can be retried', async () => {
  const leave = vi
    .spyOn(api, 'logout')
    .mockRejectedValueOnce(new api.ApiError(503))
    .mockResolvedValue(null);
  const { client } = setup(session, { ...tenant, can_manage: false });
  await screen.findByText(/Only a workspace owner/);
  expect(screen.queryByLabelText('Workspace name')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Sign out' }));
  await screen.findByRole('alert');
  await userEvent.click(
    screen.getByRole('button', { name: 'Sign out all my sessions' }),
  );
  await screen.findByText('You’re signed out.');
  expect(leave).toHaveBeenLastCalledWith(true);
  expect(client.getQueryData(['tenant', id])).toBeUndefined();
});
it('expired logout and mutation clear private data', async () => {
  vi.spyOn(api, 'logout').mockRejectedValue(new api.ApiError(401));
  const first = setup();
  await screen.findByText('Garden team');
  await userEvent.click(screen.getByRole('button', { name: 'Sign out' }));
  await screen.findByText(/Your session has ended/);
  first.unmount();
  vi.spyOn(api, 'updateTenant').mockRejectedValue(new api.ApiError(401));
  setup();
  await screen.findByLabelText('Workspace name');
  await userEvent.type(screen.getByLabelText('Workspace name'), '!');
  await userEvent.click(screen.getByRole('button', { name: 'Save name' }));
  await screen.findByLabelText('Workspace ID');
});
it('tenant errors expose recovery while never rendering cached metadata', async () => {
  vi.spyOn(api, 'getSession').mockResolvedValue(session);
  vi.spyOn(api, 'getTenant')
    .mockRejectedValueOnce(new api.ApiError(503))
    .mockResolvedValue(tenant);
  render(<App client={newClient()} />);
  await screen.findByRole('alert');
  await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
  await screen.findByText('Garden team');
});
it('expires sessions at the server deadline and rejects tenant authentication loss', async () => {
  const first = setup({
    ...session,
    expires_at: Math.floor(Date.now() / 1000) - 1,
  });
  await screen.findByLabelText('Workspace ID');
  first.unmount();
  vi.mocked(api.getSession).mockResolvedValue(session);
  vi.mocked(api.getTenant).mockRejectedValue(new api.ApiError(401));
  render(<App client={newClient()} />);
  await screen.findByLabelText('Workspace ID');
});
it('supports locale detection and blocked preference storage', () => {
  localStorage.clear();
  vi.spyOn(navigator, 'language', 'get').mockReturnValue('pt-PT');
  expect(detectedLocale()).toBe('pt-BR');
  vi.spyOn(navigator, 'language', 'get').mockReturnValue('fr');
  expect(detectedLocale()).toBe('en');
  localStorage.setItem('karate.locale', 'en');
  expect(detectedLocale()).toBe('en');
  localStorage.setItem('karate.locale', 'pt-BR');
  expect(detectedLocale()).toBe('pt-BR');
  vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
    throw new Error('blocked');
  });
  expect(detectedLocale()).toBe('en');
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
    throw new Error('blocked');
  });
  localeDocument('pt-BR');
  expect(document.documentElement.dir).toBe('ltr');
});
it('mounts the entry point and fails loudly for a missing host element', async () => {
  vi.spyOn(api, 'getSession').mockResolvedValue(null);
  const host = document.createElement('div');
  host.id = 'root';
  document.body.append(host);
  await act(async () => {
    await import('../src/main');
  });
  await screen.findByLabelText('Workspace ID');
  host.remove();
  vi.resetModules();
  await expect(import('../src/main')).rejects.toThrow(
    'Missing application root',
  );
});
it('does not repopulate tenant caches when a write finishes after logout', async () => {
  let finish: (value: api.Tenant) => void = () => {
    throw new Error('request did not start');
  };
  vi.spyOn(api, 'updateTenant').mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  vi.spyOn(api, 'logout').mockResolvedValue(null);
  const { client } = setup();
  await screen.findByLabelText('Workspace name');
  await userEvent.type(screen.getByLabelText('Workspace name'), '!');
  await userEvent.click(screen.getByRole('button', { name: 'Save name' }));
  await userEvent.click(screen.getByRole('button', { name: /^Sign out$/ }));
  await screen.findByLabelText('Workspace ID');
  await act(async () =>
    finish({ ...tenant, name: 'Late result', revision: 2 }),
  );
  expect(client.getQueryData(['tenant', id])).toBeUndefined();
  expect(client.getMutationCache().getAll()).toHaveLength(0);
});

it('offers a new workspace sign-in after membership loss', async () => {
  vi.spyOn(api, 'getSession').mockRejectedValue(new api.ApiError(403));
  render(<App client={newClient()} />);
  await screen.findByLabelText('Workspace ID');
  expect(screen.getByRole('alert')).toHaveTextContent(
    'membership no longer permits access',
  );
});

const deletion: api.Deletion = {
  request_id: id,
  state: 'pending',
  requested_at: 1800000000,
  erase_after: 1800604800,
  retention_days: 7,
};
it('requires confirmation, previews consequences, schedules and cancels deletion', async () => {
  vi.spyOn(api, 'changeDeletion').mockImplementation(
    async (current, _id, days, cancel) => ({
      ...current,
      state: 'suspended',
      revision: current.revision + 1,
      deletion: {
        ...deletion,
        retention_days: days,
        state: cancel ? 'cancelled' : 'pending',
      },
    }),
  );
  setup();
  await screen.findByLabelText('Confirm workspace ID');
  const user = userEvent.setup();
  expect(
    screen.getByRole('button', { name: 'Review deletion' }),
  ).toBeDisabled();
  fireEvent.submit(
    screen.getByLabelText('Confirm workspace ID').closest('form')!,
  );
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  await user.type(screen.getByLabelText('Confirm workspace ID'), id);
  await user.clear(screen.getByLabelText('Recovery window (days)'));
  expect(
    screen.getByRole('button', { name: 'Review deletion' }),
  ).toBeDisabled();
  await user.type(screen.getByLabelText('Recovery window (days)'), '31');
  expect(
    screen.getByRole('button', { name: 'Review deletion' }),
  ).toBeDisabled();
  await user.clear(screen.getByLabelText('Recovery window (days)'));
  await user.type(screen.getByLabelText('Recovery window (days)'), '30');
  const button = screen.getByRole('button', { name: 'Review deletion' });
  await user.click(button);
  fireEvent(screen.getByRole('dialog'), new Event('cancel'));
  await user.click(screen.getByRole('button', { name: /^Cancel$/ }));
  expect(button).toHaveFocus();
  await user.click(button);
  await user.click(screen.getByRole('button', { name: 'Schedule deletion' }));
  await screen.findByText(/Deletion is scheduled/);
  expect(
    screen.getByRole('button', { name: 'Resume workspace' }),
  ).toBeDisabled();
  expect(screen.getByLabelText('Workspace name')).toBeDisabled();
  await user.click(screen.getByRole('button', { name: 'Cancel deletion' }));
  await screen.findByText(/Deletion was cancelled/);
  expect(
    screen.getByRole('button', { name: 'Resume workspace' }),
  ).toBeEnabled();
  vi.spyOn(api, 'updateTenant').mockResolvedValue({
    ...tenant,
    revision: 4,
    deletion: { ...deletion, state: 'cancelled' },
  });
  await user.click(screen.getByRole('button', { name: 'Resume workspace' }));
  await user.click(screen.getByRole('button', { name: 'Confirm change' }));
  await screen.findByText('Active', { exact: true });
  expect(screen.queryByText(/Deletion was cancelled/)).not.toBeInTheDocument();
});
it('shows blocked/erasing states and refreshes after a deletion conflict', async () => {
  vi.spyOn(api, 'changeDeletion').mockRejectedValue(new api.ApiError(409));
  const first = setup(session, {
    ...tenant,
    state: 'suspended',
    deletion: { ...deletion, state: 'blocked_authority' },
  });
  await screen.findByText(/requester is no longer/);
  await userEvent.click(
    screen.getByRole('button', { name: 'Cancel deletion' }),
  );
  await screen.findByText(/workspace changed/);
  expect(
    screen.getByRole('button', { name: 'Cancel deletion' }),
  ).toBeDisabled();
  await userEvent.click(
    screen.getByRole('button', { name: 'Refresh workspace' }),
  );
  await waitFor(() =>
    expect(
      screen.getByRole('button', { name: 'Cancel deletion' }),
    ).toBeEnabled(),
  );
  first.unmount();
  setup(session, {
    ...tenant,
    state: 'suspended',
    deletion: { ...deletion, state: 'erasing' },
  });
  await screen.findByText(/Erasure has started/);
  expect(
    screen.queryByRole('button', { name: 'Cancel deletion' }),
  ).not.toBeInTheDocument();
});
it('clears an expired session during deletion cancellation', async () => {
  vi.spyOn(api, 'changeDeletion').mockRejectedValue(new api.ApiError(401));
  setup(session, { ...tenant, state: 'suspended', deletion });
  await screen.findByText(/Deletion is scheduled/);
  await userEvent.click(
    screen.getByRole('button', { name: 'Cancel deletion' }),
  );
  await screen.findByText(/Your session has ended/);
});
