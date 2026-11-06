import { useRef, useState, type FormEvent } from 'react';
import { useMutation } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  ApiError,
  changeDeletion,
  errorKey,
  workspaceId,
  type Tenant,
} from './api';
export function DeletionPanel({
  tenant,
  onSaved,
  onExpired,
  refresh,
}: {
  tenant: Tenant;
  onSaved: (value: Tenant) => void;
  onExpired: () => void;
  refresh: () => Promise<void>;
}) {
  const { t, i18n } = useTranslation();
  const [confirmation, setConfirmation] = useState('');
  const [days, setDays] = useState(7);
  const [blocked, setBlocked] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const job = tenant.deletion;
  const active = job !== null && job.state !== 'cancelled';
  const valid =
    workspaceId(confirmation) === tenant.tenant_id &&
    Number.isInteger(days) &&
    days >= 7 &&
    days <= 30;
  const change = useMutation({
    mutationFn: (cancel: boolean) =>
      changeDeletion(
        tenant,
        cancel ? tenant.tenant_id : confirmation.trim().toLowerCase(),
        cancel ? 7 : days,
        cancel,
      ),
    onSuccess: onSaved,
    onError: (error) => {
      if (error instanceof ApiError && error.status === 401) onExpired();
      else setBlocked(true);
    },
  });
  const close = () => {
    dialog.current?.close();
    trigger.current?.focus();
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (valid) dialog.current?.showModal();
  };
  return (
    <section className="deletion">
      <h3>{t('deletionTitle')}</h3>
      <p className="muted">{t('deletionIntro')}</p>
      {change.isError && (
        <p className="notice error" role="alert">
          {t(errorKey(change.error))}
        </p>
      )}
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
      {active ? (
        <div className="notice">
          <p role="status">
            {t(
              job.state === 'blocked_authority'
                ? 'deletionBlocked'
                : job.state === 'erasing'
                  ? 'deletionErasing'
                  : 'deletionPending',
            )}
          </p>
          <p>
            {t('deletionDue', {
              time: new Intl.DateTimeFormat(i18n.language, {
                dateStyle: 'medium',
                timeStyle: 'short',
              }).format(new Date(job.erase_after * 1000)),
            })}
          </p>
          {job.state !== 'erasing' && (
            <button
              disabled={change.isPending || blocked}
              onClick={() => change.mutate(true)}
            >
              {t(change.isPending ? 'saving' : 'deletionCancel')}
            </button>
          )}
        </div>
      ) : (
        <>
          {job?.state === 'cancelled' && tenant.state === 'suspended' && (
            <p className="notice" role="status">
              {t('deletionCancelled')}
            </p>
          )}
          <form onSubmit={submit}>
            <label htmlFor="deletion-confirmation">
              {t('deletionConfirmation')}
            </label>
            <input
              id="deletion-confirmation"
              autoComplete="off"
              spellCheck={false}
              maxLength={64}
              value={confirmation}
              onChange={(e) => setConfirmation(e.target.value)}
              disabled={change.isPending || blocked}
              aria-describedby="deletion-hint"
            />
            <p id="deletion-hint" className="hint">
              {t('deletionHint', { id: tenant.tenant_id })}
            </p>
            <label htmlFor="retention-days">{t('retentionDays')}</label>
            <input
              id="retention-days"
              type="number"
              min={7}
              max={30}
              step={1}
              value={Number.isNaN(days) ? '' : days}
              onChange={(e) => setDays(e.target.valueAsNumber)}
              disabled={change.isPending || blocked}
              aria-describedby="retention-hint"
            />
            <p id="retention-hint" className="hint">
              {t('retentionHint')}
            </p>
            <button
              ref={trigger}
              className="danger"
              disabled={!valid || change.isPending || blocked}
            >
              {t(change.isPending ? 'saving' : 'deletionReview')}
            </button>
          </form>
        </>
      )}
      <dialog
        ref={dialog}
        aria-labelledby="deletion-dialog-title"
        aria-describedby="deletion-dialog-detail"
        onCancel={() => trigger.current?.focus()}
      >
        <h2 id="deletion-dialog-title">{t('deletionDialogTitle')}</h2>
        <p id="deletion-dialog-detail">{t('deletionDialogDetail', { days })}</p>
        <div className="dialog-actions">
          <button onClick={close}>{t('cancel')}</button>
          <button
            className="danger"
            onClick={() => {
              close();
              change.mutate(false);
            }}
          >
            {t('deletionSchedule')}
          </button>
        </div>
      </dialog>
    </section>
  );
}
