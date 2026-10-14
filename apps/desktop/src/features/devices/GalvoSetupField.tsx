import { useId, useState } from 'react';
import { getApi } from '@/api';
import type { DeviceProfile, GalvoSafetyAck, GalvoVerifyReport, LaserSource } from '@/api/types';
import { GalvoExperimentalBadge } from '@/features/machine/GalvoNotice';
import { GALVO_VID_PID, GALVO_VIRTUAL, galvoBoardNumber, isGalvoUsbPort, SOURCE_WAVELENGTH_NM } from '@/features/machine/galvoPorts';
import { COMMON_FIELDS_MM, GALVO_SOURCES } from '@/features/machine/galvoProfile';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';

/** Lo que el asistente pregunta de un galvo: el campo de la lente, la fuente y la tarjeta a verificar. */
export interface GalvoSetup {
  field_mm: number;
  source: LaserSource;
  /** Puerto de la tarjeta a verificar (`usb:galvo:<n>` o `virtual:galvo`); vacío = la primera USB, o la simulada. */
  port?: string;
}

export const DEFAULT_GALVO_SETUP: GalvoSetup = { field_mm: 110, source: 'fiber_q' };

/** El puerto que se usa: el elegido, si no la primera tarjeta USB encontrada y, si no hay, la simulada. */
export function setupPort(setup: GalvoSetup, boards: readonly string[]): string {
  return setup.port && (setup.port === GALVO_VIRTUAL || boards.includes(setup.port)) ? setup.port : (boards[0] ?? GALVO_VIRTUAL);
}

/** El perfil con la lente, la fuente y la cama (el campo) que eligió el asistente. */
export function applySetup(profile: DeviceProfile, setup: GalvoSetup): DeviceProfile {
  return {
    ...profile,
    bed: [setup.field_mm, setup.field_mm],
    galvo: { ...profile.galvo, field_mm: setup.field_mm, source: setup.source },
  };
}

const CHECK_TEXT: Record<string, I18nKey> = {
  board: 'galvo.wizard.verify.check.board',
  idle: 'galvo.wizard.verify.check.idle',
  frame: 'galvo.wizard.verify.check.frame',
  interlock: 'galvo.wizard.verify.check.interlock',
};

export type VerifyState =
  | { status: 'idle' }
  | { status: 'running' }
  | { status: 'cancelled' }
  | { status: 'error'; message: string }
  | { status: 'done'; report: GalvoVerifyReport };

/**
 * Verificación SIN LÁSER del perfil con la tarjeta elegida. Una tarjeta USB real pasa antes por el
 * reconocimiento de seguridad (`requestAck`), que se pide en CADA verificación; si se cancela, no se abre.
 */
export async function verifySetup(
  profile: DeviceProfile,
  setup: GalvoSetup,
  boards: readonly string[],
  requestAck: () => Promise<GalvoSafetyAck | null>,
): Promise<VerifyState> {
  const port = setupPort(setup, boards);
  try {
    const ack = isGalvoUsbPort(port) ? await requestAck() : null;
    if (isGalvoUsbPort(port) && !ack) return { status: 'cancelled' };
    return { status: 'done', report: await getApi().galvoVerify({ device: applySetup(profile, setup), port, galvo_ack: ack }) };
  } catch (e) {
    return { status: 'error', message: errorMessage(e) };
  }
}

/** Resultado de la verificación: lo que leyó la tarjeta y cada comprobación con su resultado. */
export function VerifyResult({ report }: { report: GalvoVerifyReport }) {
  const ok = report.checks.every((c) => c.ok);
  const id = `${report.vendor_id.toString(16).toUpperCase()}:${report.product_id.toString(16).toUpperCase().padStart(4, '0')}`;
  return (
    <div className="dev-galvo-verify" role="status">
      <p className="hint num">{t('galvo.wizard.verify.board', { board: report.board, id, serial: report.serial, version: report.version })}</p>
      <ul className="mc-list">
        {report.checks.map((c) => (
          <li key={c.id} data-ok={c.ok}>
            <strong>{CHECK_TEXT[c.id] ? t(CHECK_TEXT[c.id]!) : c.id}</strong>
            {' · '}
            <span className={c.ok ? undefined : 'hint--danger'}>{c.ok ? '✓' : '✗'} {c.detail}</span>
          </li>
        ))}
      </ul>
      <p className={ok ? 'hint' : 'warn-line'} role={ok ? undefined : 'alert'}>{t(ok ? 'galvo.wizard.verify.ok' : 'galvo.wizard.verify.bad')}</p>
    </div>
  );
}

interface Props {
  setup: GalvoSetup;
  onSetup: (s: GalvoSetup) => void;
  /** El perfil que se está creando: lo necesita la verificación para comprobar que se compila. */
  profile?: DeviceProfile;
  /** Tarjetas USB encontradas (`usb:galvo:<n>`). */
  boards?: readonly string[];
}

/**
 * Asistente «Agregar máquina» con un galvo, en cuatro pasos: la lente (y su fuente), el campo, la tarjeta y la
 * verificación SIN LÁSER. Esta última lee la tarjeta (con una USB real pide antes la confirmación de seguridad) y
 * comprueba que el perfil se compila; no mueve los espejos ni enciende el puntero ni la fuente.
 */
export function GalvoSetupField({ setup, onSetup, profile, boards = [] }: Props) {
  const id = useId();
  const field = COMMON_FIELDS_MM.includes(setup.field_mm as (typeof COMMON_FIELDS_MM)[number]) ? String(setup.field_mm) : 'custom';
  const port = setupPort(setup, boards);
  const requestAck = useStore((s) => s.requestGalvoAck);
  const [verify, setVerify] = useState<VerifyState>({ status: 'idle' });
  const nm = SOURCE_WAVELENGTH_NM[setup.source];
  // Cambiar algo borra un resultado que ya no vale.
  const change = (next: GalvoSetup) => {
    setVerify({ status: 'idle' });
    onSetup(next);
  };
  const run = async () => {
    if (!profile) return;
    setVerify({ status: 'running' });
    setVerify(await verifySetup(profile, setup, boards, requestAck));
  };
  return (
    <section className="field dev-galvo-setup" aria-label={t('galvo.wizard.setup')}>
      <p className="field__label">
        {t('galvo.wizard.setup')} <GalvoExperimentalBadge />
      </p>
      <ol className="dev-galvo-steps" aria-label={t('galvo.wizard.steps')}>
        <li>
          <h4 className="mc-sub">{`1 · ${t('galvo.wizard.lens.title')}`}</h4>
          <label htmlFor={`${id}-source`} className="field__label">{t('galvo.dev.source')}</label>
          <select id={`${id}-source`} className="select" value={setup.source} aria-describedby={`${id}-source-help`} onChange={(e) => change({ ...setup, source: e.target.value as LaserSource })}>
            {GALVO_SOURCES.map((s) => (
              <option key={s} value={s}>{t(`galvo.source.${s}`)}</option>
            ))}
          </select>
          <p id={`${id}-source-help`} className="hint">{t(setup.source === 'fiber_mopa' ? 'galvo.wizard.source.mopa' : 'galvo.wizard.source.help')}</p>
          <p className="hint num">{t('galvo.wizard.lens.help', { nm })}</p>
        </li>
        <li>
          <h4 className="mc-sub">{`2 · ${t('galvo.wizard.field.title')}`}</h4>
          <label htmlFor={`${id}-field`} className="field__label">{t('galvo.dev.field')}</label>
          <select id={`${id}-field`} className="select" value={field} aria-describedby={`${id}-field-help`} onChange={(e) => e.target.value !== 'custom' && change({ ...setup, field_mm: Number(e.target.value) })}>
            {COMMON_FIELDS_MM.map((mm) => (
              <option key={mm} value={mm}>{`${mm} × ${mm} mm`}</option>
            ))}
            {field === 'custom' && <option value="custom">{`${setup.field_mm} × ${setup.field_mm} mm`}</option>}
          </select>
          <p id={`${id}-field-help`} className="hint">{t('galvo.wizard.field.help')}</p>
        </li>
        <li>
          <h4 className="mc-sub">{`3 · ${t('galvo.wizard.board.title')}`}</h4>
          <label htmlFor={`${id}-board`} className="field__label">{t('galvo.wizard.board.pick')}</label>
          <select id={`${id}-board`} className="select" value={port} aria-describedby={`${id}-board-help`} onChange={(e) => change({ ...setup, port: e.target.value })}>
            {boards.map((b) => (
              <option key={b} value={b}>{`${t('galvo.port.usb', { n: galvoBoardNumber(b) })} · ${GALVO_VID_PID}`}</option>
            ))}
            <option value={GALVO_VIRTUAL}>{t('galvo.wizard.board.virtual')}</option>
          </select>
          <p id={`${id}-board-help`} className="hint">{t('galvo.wizard.board.help')}</p>
          {boards.length === 0 && <p className="hint" role="status">{t('galvo.wizard.board.none')}</p>}
        </li>
        <li>
          <h4 className="mc-sub">{`4 · ${t('galvo.wizard.verify.title')}`}</h4>
          <p className="hint">{t('galvo.wizard.verify.help')}</p>
          <button type="button" className="btn" disabled={!profile || verify.status === 'running'} onClick={() => void run()}>
            {t(verify.status === 'running' ? 'galvo.wizard.verify.running' : 'galvo.wizard.verify.run')}
          </button>
          {verify.status === 'cancelled' && <p className="hint" role="status">{t('galvo.wizard.verify.cancelled')}</p>}
          {verify.status === 'error' && <p className="warn-line" role="alert">{t('galvo.wizard.verify.error', { error: verify.message })}</p>}
          {verify.status === 'done' && <VerifyResult report={verify.report} />}
          <p className="hint">{t('galvo.wizard.verify.after')}</p>
        </li>
      </ol>
      <p className="hint">{t('galvo.wizard.defaults')}</p>
      <p className="hint">{t('galvo.experimental.note')}</p>
    </section>
  );
}
