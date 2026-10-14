import { TriangleAlert } from 'lucide-react';
import type { DeviceProfile } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { usePref } from '@/store';
import type { RuidaStartGate } from './ruidaStart';

/** Texto del botón de confirmar el envío: con Ruida dice si el trabajo se ejecuta o solo se guarda. */
export function ruidaConfirmKey(device: Pick<DeviceProfile, 'kind' | 'ruida'>): I18nKey | null {
  if (device.kind !== 'ruida') return null;
  return device.ruida.run_after_upload ? 'ruida.start.confirmRun' : 'ruida.start.confirmStore';
}

/**
 * Aviso del diálogo de inicio con una máquina Ruida: es experimental y dice con claridad si el
 * trabajo EMPIEZA a ejecutarse al llegar o solo se guarda como documento en la controladora. Si se
 * guarda, deja elegir el nombre del documento (M23) y exige confirmar si va a reemplazar a otro.
 */
export function RuidaStartNotice({ device, gate }: { device: Pick<DeviceProfile, 'kind' | 'ruida'>; gate?: RuidaStartGate }) {
  if (device.kind !== 'ruida') return null;
  return (
    <section className="warn-line" role="note" aria-label={t('ruida.start.title')}>
      <TriangleAlert size={14} aria-hidden="true" />
      <div>
        <p><strong>{t('ruida.start.title')}.</strong> {t('ruida.experimental.note')}</p>
        <p className="warn-line__key">{device.ruida.run_after_upload ? t('ruida.start.run') : t('ruida.start.store')}</p>
        {gate?.applies && <RuidaNameField gate={gate} />}
      </div>
    </section>
  );
}

/** Campo del nombre del documento y, si ya existe uno igual, la casilla para reemplazarlo. */
function RuidaNameField({ gate }: { gate: RuidaStartGate }) {
  const writesOn = usePref('ruidaExperimentalWrites');
  return (
    <div className="ruida-name">
      <label className="field">
        <span className="field__label">{t('ruida.name.label')}</span>
        <input
          className="input num"
          type="text"
          maxLength={8}
          value={gate.name}
          placeholder={t('ruida.name.placeholder')}
          aria-invalid={gate.nameValid ? undefined : true}
          onChange={(e) => gate.setName(e.target.value)}
        />
      </label>
      <p className="hint">{t('ruida.name.help')}</p>
      {!gate.nameValid && <p className="hint hint--danger" role="alert">{t('ruida.name.bad')}</p>}
      {gate.checking && <p className="hint" role="status">{t('ruida.name.checking')}</p>}
      {gate.checkError && <p className="hint" role="status">{t('ruida.name.checkFailed', { error: gate.checkError })}</p>}
      {gate.clash && (
        <div role="alert">
          <p className="hint hint--danger">{t('ruida.name.exists', { name: gate.clash.name ?? gate.name, index: gate.clash.index })}</p>
          <label className="mc-check">
            <input type="checkbox" checked={gate.overwrite} disabled={!writesOn} title={writesOn ? undefined : t('ruida.exp.off')} onChange={(e) => gate.setOverwrite(e.target.checked)} />
            {t('ruida.name.overwrite', { name: gate.clash.name ?? gate.name })}
          </label>
          {!gate.overwrite && <p className="hint hint--danger">{t('ruida.name.overwriteNeeded')}</p>}
        </div>
      )}
    </div>
  );
}
