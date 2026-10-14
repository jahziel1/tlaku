import { MapPin, X } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import type { StartFrom } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { useJobPlacement } from './hooks';
import { ANCHOR_KEYS, OriginPicker } from './OriginPicker';
import { OriginIllustration } from './OriginIllustration';
import type { Placement } from './placement';

const MODES: { id: StartFrom; title: I18nKey; desc: I18nKey }[] = [
  { id: 'absolute', title: 'machine.startFrom.absolute', desc: 'machine.startFrom.absolute.desc' },
  { id: 'current_position', title: 'machine.startFrom.current', desc: 'machine.startFrom.current.desc' },
  { id: 'user_origin', title: 'machine.startFrom.user', desc: 'machine.startFrom.user.desc' },
];

/** Frase que dice, con coordenadas, dónde quedará el trabajo (o qué falta para saberlo). */
export function placementSummary(p: Placement, startFrom: StartFrom, jobOrigin: number): { text: string; problem: boolean } {
  if (p.status === 'noDesign') return { text: t('machine.place.noDesign'), problem: false };
  if (p.status === 'needHead') return { text: t('machine.place.needHead'), problem: true };
  if (p.status === 'needUserOrigin') return { text: t('machine.place.needUserOrigin'), problem: true };
  const [[x0, y0], [x1, y1]] = p.rect!;
  const range = { x0: fmt(x0, 1), x1: fmt(x1, 1), y0: fmt(y0, 1), y1: fmt(y1, 1) };
  let text =
    startFrom === 'absolute'
      ? t('machine.place.absolute', range)
      : t(startFrom === 'current_position' ? 'machine.place.current' : 'machine.place.user', {
          ...range,
          anchor: t(ANCHOR_KEYS[jobOrigin]!),
        });
  if (!p.fits) {
    const sides = (
      [
        ['left', 'machine.side.left'],
        ['right', 'machine.side.right'],
        ['top', 'machine.side.top'],
        ['bottom', 'machine.side.bottom'],
      ] as const
    )
      .filter(([k]) => p.overflow[k] > 1e-6)
      .map(([k, key]) => `${t(key)} ${fmt(p.overflow[k], 1)} mm`)
      .join(', ');
    text += ` ${t('machine.place.overflow', { sides })}`;
  }
  return { text, problem: !p.fits };
}

/**
 * "Iniciar desde": tres modos y los 9 puntos del trabajo, con la ilustración de la cama que
 * muestra, con las mismas coordenadas que usará el motor, dónde quedará el trabajo.
 */
export function StartFromSection() {
  const startFrom = useStore((s) => s.jobOptions.start_from);
  const jobOrigin = useStore((s) => s.jobOptions.job_origin);
  const userOrigin = useStore(useShallow((s) => s.jobOptions.user_origin));
  const setJobOptions = useStore((s) => s.setJobOptions);
  const device = useStore(useShallow((s) => {
    const d = selectActiveDevice(s);
    return { bed: d.bed, origin: d.origin, swap_xy: d.swap_xy, rotary: d.rotary };
  }));
  const head = useStore(useShallow((s) => (s.connection.connected && s.status ? s.status.work_pos : null)));
  const connected = useStore((s) => s.connection.connected);
  const setHere = useStore((s) => s.setUserOriginHere);
  const clearOrigin = useStore((s) => s.clearUserOrigin);
  const placement = useJobPlacement();
  const summary = placementSummary(placement, startFrom, jobOrigin);

  return (
    <div className="mc-startfrom">
      <div className="mc-modes" role="radiogroup" aria-label={t('machine.startFrom')}>
        {MODES.map((m) => (
          <button
            key={m.id}
            type="button"
            role="radio"
            aria-checked={startFrom === m.id}
            className="mc-mode"
            onClick={() => setJobOptions({ start_from: m.id })}
          >
            <span className="mc-mode__title">{t(m.title)}</span>
            <span className="mc-mode__desc">{t(m.desc)}</span>
          </button>
        ))}
      </div>

      <div className="mc-anchor">
        <OriginPicker
          value={jobOrigin}
          onChange={(job_origin) => setJobOptions({ job_origin })}
          disabled={startFrom === 'absolute'}
          reason={t('machine.anchor.disabled')}
        />
        <p className="hint">{startFrom === 'absolute' ? t('machine.anchor.disabled') : t('machine.anchor.help', { anchor: t(ANCHOR_KEYS[jobOrigin]!) })}</p>
      </div>

      {startFrom === 'user_origin' && (
        <div className="mc-userorigin">
          <p className="hint num">
            {userOrigin ? t('machine.origin.current', { x: fmt(userOrigin[0], 2), y: fmt(userOrigin[1], 2) }) : t('machine.origin.none')}
          </p>
          <div className="row">
            <button type="button" className="btn grow" disabled={!connected} title={connected ? t('machine.origin.setHint') : t('reason.needMachine')} onClick={setHere}>
              <MapPin size={14} aria-hidden="true" />{t('machine.origin.set')}
            </button>
            {userOrigin && (
              <button type="button" className="btn" title={t('machine.origin.clear')} aria-label={t('machine.origin.clear')} onClick={clearOrigin}>
                <X size={14} aria-hidden="true" />
              </button>
            )}
          </div>
        </div>
      )}

      <OriginIllustration device={device} placement={placement} jobOrigin={jobOrigin} head={head} userOrigin={userOrigin} />
      <p className={summary.problem ? 'hint hint--danger' : 'hint'} role="status">{summary.text}</p>
    </div>
  );
}
