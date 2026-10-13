import { t } from '@/i18n';
import { usePref } from '@/store';
import { PrefCheck, PrefNumber } from '@/features/settings/fields';
import { useAutomationStatus } from './automation';

/** Sección «Automatización» de Preferencias: servidor UDP local, apagado por omisión. */
export function AutomationPanel() {
  const enabled = usePref('automationEnabled');
  const status = useAutomationStatus((s) => s.status);
  return (
    <>
      <fieldset className="set-group">
        <PrefCheck k="automationEnabled" label="prefs.automation.enabled" />
        <p className="hint">{t('prefs.automation.hint')}</p>
        {enabled && (
          <>
            <div className="set-grid">
              <PrefNumber k="automationPort" label="prefs.automation.port" unit="prefs.automation.portUnit" />
            </div>
            <p role="status" className={status?.error ? 'hint hint--danger' : 'hint'}>
              {status?.error
                ? t('prefs.automation.status.error', { error: status.error })
                : status?.running
                  ? t('prefs.automation.status.on', { port: status.port })
                  : t('prefs.automation.status.off')}
            </p>
          </>
        )}
      </fieldset>
      <fieldset className="set-group">
        <PrefCheck k="automationAllowStart" label="prefs.automation.allowStart" />
        <p className="hint">{t('prefs.automation.allowStart.hint')}</p>
      </fieldset>
    </>
  );
}
