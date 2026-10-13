import { useState } from 'react';
import type { DeviceProfile } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { isBadRotaryTest, openRotaryTestAsNewDocument, ROTARY_TEST_POWER, ROTARY_TEST_SPEED } from './DeviceActions';
import { Field, NumInput } from './DeviceFields';

const CHECKS: I18nKey[] = ['devices.rotary.test.check1', 'devices.rotary.test.check2', 'devices.rotary.test.check3', 'devices.rotary.test.check4'];

/**
 * «Probar una vuelta»: genera con el motor una línea del largo de una vuelta de la pieza y la
 * abre como documento nuevo. `blocked` es el motivo (texto) por el que ahora no se puede, o `null`.
 */
export function RotaryTest({ draft, blocked }: { draft: DeviceProfile; blocked: I18nKey | null }) {
  const close = useStore((s) => s.closeDialog);
  const [speed, setSpeed] = useState(ROTARY_TEST_SPEED);
  const [power, setPower] = useState(ROTARY_TEST_POWER);
  const [busy, setBusy] = useState(false);
  const bad = isBadRotaryTest(speed, power);
  const reason: I18nKey | null = blocked ?? (bad ? 'devices.rotary.test.invalid' : null);

  const run = async () => {
    if (reason || busy) return;
    setBusy(true);
    const ok = await openRotaryTestAsNewDocument(draft, speed, power);
    setBusy(false);
    // El documento nuevo está detrás del diálogo: se cierra para verlo.
    if (ok) close();
  };

  return (
    <section className="dv-test" aria-label={t('devices.rotary.test.title')}>
      <h3 className="mc-sub">{t('devices.rotary.test.title')}</h3>
      <p className="hint">{t('devices.rotary.test.intro')}</p>
      <div className="field-grid">
        <Field label={t('devices.rotary.test.speed')} help="devices.rotary.test.speed.help">
          {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm/min" value={speed} describedBy={describedBy} invalid={invalid || speed <= 0} onCommit={setSpeed} />}
        </Field>
        <Field label={t('devices.rotary.test.power')} help="devices.rotary.test.power.help">
          {({ id, describedBy, invalid }) => <NumInput id={id} unit="%" value={power} describedBy={describedBy} invalid={invalid || power <= 0 || power > 100} onCommit={setPower} />}
        </Field>
      </div>
      <div className="row">
        <button type="button" className="btn btn--accent" disabled={reason !== null || busy} title={reason ? t(reason) : undefined} onClick={() => void run()}>
          {busy ? t('devices.rotary.test.running') : t('devices.rotary.test.run')}
        </button>
        {reason && <span className="hint" role="status">{t(reason)}</span>}
      </div>
      <h4 className="mc-sub">{t('devices.rotary.test.check')}</h4>
      <ul className="dv-checks">
        {CHECKS.map((k) => (
          <li key={k} className="hint">{t(k)}</li>
        ))}
      </ul>
    </section>
  );
}
