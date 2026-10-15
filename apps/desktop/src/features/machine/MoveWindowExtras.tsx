import { ArrowDownToLine, ArrowUpToLine, Crosshair, Flame, MapPin, Save, Trash2, TriangleAlert } from 'lucide-react';
import { useId, useState } from 'react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { jogBlockedReason } from '@/store/machine';
import { FIRE_LIMITS, U_LETTERS, type ULetter } from '@/store/moveControl';
import { NumberField } from '@/ui/NumberField';
import { LhyNotApplicable } from './LhyNotice';
import { GalvoNotApplicable } from './GalvoNotice';
import { selectIsGalvo } from './galvoPorts';
import { selectIsLhy } from './lhyPorts';
import { MAX_POSITIONS } from './positions';
import { Modal } from './Modal';

/** Jog de los ejes extra: Z (con «Enfocar») y U, solo si el perfil los declara. */
export function ExtraAxesControls() {
  const device = useStore(selectActiveDevice);
  const whyKey = useStore(jogBlockedReason);
  const why = whyKey ? t(whyKey) : undefined;
  const step = useStore((s) => s.jogStep);
  const jogExtra = useStore((s) => s.jogExtra);
  const focusZ = useStore((s) => s.focusZ);
  const setFocusZ = useStore((s) => s.setFocusZ);
  const goFocus = useStore((s) => s.goFocus);
  const uLetter = useStore((s) => s.uLetter);
  const setULetter = useStore((s) => s.setULetter);
  const letterId = useId();
  const lhy = useStore(selectIsLhy);
  const galvo = useStore(selectIsGalvo);
  // El galvo no tiene Z ni ejes extra: la mesa y el rotativo se mueven a mano.
  if (galvo) {
    return (
      <div className="mc-axes">
        <h4 className="mc-sub">{t('machine.axes')}</h4>
        <GalvoNotApplicable reason="galvo.axes.reason" />
      </div>
    );
  }
  // La M2 Nano no tiene Z, U ni aire: se dice por qué en lugar de los controles.
  if (lhy) {
    return (
      <div className="mc-axes">
        <h4 className="mc-sub">{t('machine.axes')}</h4>
        <LhyNotApplicable reason="lhymicro.axes.reason" />
      </div>
    );
  }
  if (!device.z_axis && !device.u_axis) return null;
  const disabled = whyKey !== null;

  const axisRow = (axis: 'z' | 'u', label: string) => (
    <div className="row row--center" role="group" aria-label={label}>
      <span className="field__label grow">{label}</span>
      <button type="button" className="btn" disabled={disabled} title={why ?? t('machine.axis.up', { axis: label, step: fmt(step, 2) })} onClick={() => void jogExtra(axis, 1)}>
        <ArrowUpToLine size={14} aria-hidden="true" />{t('machine.axis.upShort')}
      </button>
      <button type="button" className="btn" disabled={disabled} title={why ?? t('machine.axis.down', { axis: label, step: fmt(step, 2) })} onClick={() => void jogExtra(axis, -1)}>
        <ArrowDownToLine size={14} aria-hidden="true" />{t('machine.axis.downShort')}
      </button>
    </div>
  );

  return (
    <div className="mc-axes">
      <h4 className="mc-sub">{t('machine.axes')}</h4>
      {device.z_axis && axisRow('z', t('machine.axis.z'))}
      {device.z_axis && (
        <>
          <div className="row">
            <div className="grow">
              <NumberField label={t('machine.focus.z')} unit="mm" value={focusZ} step={0.5} min={-1000} max={1000} onCommit={setFocusZ} />
            </div>
            <button type="button" className="btn" disabled={disabled} title={why ?? t('machine.focus.help')} onClick={() => void goFocus()}>
              <Crosshair size={14} aria-hidden="true" />{t('machine.focus')}
            </button>
          </div>
          <p className="hint">{t('machine.focus.help')}</p>
        </>
      )}
      {device.u_axis && axisRow('u', t('machine.axis.u'))}
      {device.u_axis && (
        <div className="row">
          <label htmlFor={letterId} className="field__label grow">{t('machine.axis.uLetter')}</label>
          <select id={letterId} className="select" value={uLetter} onChange={(e) => setULetter(e.target.value as ULetter)}>
            {U_LETTERS.map((l) => (
              <option key={l} value={l}>{l}</option>
            ))}
          </select>
        </div>
      )}
    </div>
  );
}

/** Posiciones guardadas del perfil: guardar la actual con nombre, ir a una o borrarla. */
export function SavedPositions() {
  const device = useStore(selectActiveDevice);
  const connected = useStore((s) => s.connection.connected);
  const whyKey = useStore(jogBlockedReason);
  const save = useStore((s) => s.savePosition);
  const remove = useStore((s) => s.deletePosition);
  const go = useStore((s) => s.goToPosition);
  const [name, setName] = useState('');
  const id = useId();
  const full = device.positions.length >= MAX_POSITIONS && !device.positions.some((p) => p.name === name.trim());

  return (
    <div className="mc-positions">
      <h4 className="mc-sub">{t('machine.positions')}</h4>
      <div className="row">
        <label htmlFor={id} className="sr-only">{t('machine.positions.name')}</label>
        <input id={id} className="input grow" value={name} maxLength={40} placeholder={t('machine.positions.name')} onChange={(e) => setName(e.target.value)} />
        <button
          type="button"
          className="btn"
          disabled={!connected || name.trim() === '' || full}
          title={!connected ? t('reason.needMachine') : full ? t('machine.positions.full', { max: MAX_POSITIONS }) : t('machine.positions.saveHint')}
          onClick={() => void save(name).then((ok) => ok && setName(''))}
        >
          <Save size={14} aria-hidden="true" />{t('machine.positions.save')}
        </button>
      </div>
      {device.positions.length === 0 ? (
        <p className="hint">{t('machine.positions.empty')}</p>
      ) : (
        <ul className="mc-poslist">
          {device.positions.map((p) => (
            <li key={p.name} className="row row--center">
              <span className="grow">
                {p.name} <span className="hint num">({fmt(p.x, 1)}, {fmt(p.y, 1)})</span>
              </span>
              <button type="button" className="btn" disabled={whyKey !== null} title={whyKey ? t(whyKey) : t('machine.positions.goHint', { name: p.name })} onClick={() => void go(p.name)}>
                <MapPin size={14} aria-hidden="true" />{t('machine.positions.go')}
              </button>
              <button type="button" className="icon-btn" title={t('machine.positions.delete')} aria-label={t('machine.positions.deleteN', { name: p.name })} onClick={() => void remove(p.name)}>
                <Trash2 size={14} aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** Diálogo del disparo de prueba: advertencias y una casilla obligatoria (segunda confirmación). */
function FireDialog({ onClose }: { onClose: () => void }) {
  const fire = useStore((s) => s.fire);
  const firing = useStore((s) => s.firing);
  const run = useStore((s) => s.fireTest);
  const cancel = useStore((s) => s.cancelFire);
  const [ready, setReady] = useState(false);
  const close = () => {
    if (firing) void cancel();
    onClose();
  };
  return (
    <Modal
      title={t('machine.fire.title')}
      tone="danger"
      onClose={close}
      footer={
        <>
          <button type="button" className="btn" data-autofocus onClick={close}>{firing ? t('machine.fire.stopNow') : t('machine.dlg.cancel')}</button>
          <button type="button" className="btn btn--danger" disabled={!ready || firing} onClick={() => void run().then(onClose)}>
            <Flame size={14} aria-hidden="true" />{firing ? t('machine.fire.firing') : t('machine.fire.go')}
          </button>
        </>
      }
    >
      <p>{t('machine.fire.text', { pct: fmt(fire.powerPct, 1), ms: fire.durationMs })}</p>
      <p className="warn-line">
        <TriangleAlert size={14} aria-hidden="true" />
        <span>{t('machine.fire.warning')}</span>
      </p>
      <p className="hint">{t('machine.fire.timing')}</p>
      <label className="mc-check">
        <input type="checkbox" checked={ready} disabled={firing} onChange={(e) => setReady(e.target.checked)} />
        <span>{t('machine.fire.ack')}</span>
      </label>
    </Modal>
  );
}

/** Disparo de prueba: potencia baja y duración corta configurables; pide doble confirmación. */
export function FireControls() {
  const fire = useStore((s) => s.fire);
  const setFire = useStore((s) => s.setFire);
  const whyKey = useStore(jogBlockedReason);
  const [open, setOpen] = useState(false);
  const lhy = useStore(selectIsLhy);
  const galvo = useStore(selectIsGalvo);
  // El disparo de prueba es de GRBL (M3/M5): el galvo no lo tiene (para ver dónde caerá la marca hay un recorrido en rojo).
  if (galvo) {
    return (
      <div className="mc-fire">
        <h4 className="mc-sub">{t('machine.fire')}</h4>
        <GalvoNotApplicable reason="galvo.fire.reason" />
      </div>
    );
  }
  // La M2 Nano no controla la potencia por software.
  if (lhy) {
    return (
      <div className="mc-fire">
        <h4 className="mc-sub">{t('machine.fire')}</h4>
        <LhyNotApplicable reason="lhymicro.fire.reason" />
      </div>
    );
  }
  return (
    <div className="mc-fire">
      <h4 className="mc-sub">{t('machine.fire')}</h4>
      <div className="field-grid">
        <NumberField label={t('machine.fire.power')} unit="%" value={fire.powerPct} step={0.5} min={0.1} max={FIRE_LIMITS.maxPct} onCommit={(powerPct) => setFire({ powerPct })} />
        <NumberField label={t('machine.fire.duration')} unit="ms" value={fire.durationMs} step={50} min={FIRE_LIMITS.minMs} max={FIRE_LIMITS.maxMs} onCommit={(durationMs) => setFire({ durationMs })} />
      </div>
      <p className="hint">{t('machine.fire.help', { pct: FIRE_LIMITS.maxPct, ms: FIRE_LIMITS.maxMs })}</p>
      <p className="warn-line">
        <TriangleAlert size={14} aria-hidden="true" />
        <span>{t('machine.fire.panelWarn')}</span>
      </p>
      <button type="button" className="btn" disabled={whyKey !== null} title={whyKey ? t(whyKey) : t('machine.fire.openHint')} onClick={() => setOpen(true)}>
        <Flame size={14} aria-hidden="true" />{t('machine.fire.open')}
      </button>
      {open && <FireDialog onClose={() => setOpen(false)} />}
    </div>
  );
}

/** Todo lo que la ventana Mover suma a los pasos básicos: ejes extra, posiciones guardadas y disparo de prueba. */
export function MoveWindowExtras() {
  return (
    <>
      <ExtraAxesControls />
      <SavedPositions />
      <FireControls />
    </>
  );
}
