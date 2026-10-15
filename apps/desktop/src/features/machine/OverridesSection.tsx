import { Minus, Plus, RotateCcw } from 'lucide-react';
import type { OverrideKind } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { overrideBlockedReason } from '@/store/machine';
import { NumberField } from '@/ui/NumberField';
import { PanelSection } from '@/ui/PanelSection';
import { RuidaNotApplicable } from './RuidaSection';
import { GalvoNotApplicable } from './GalvoNotice';
import { LhyNotApplicable } from './LhyNotice';
import { selectIsGalvo } from './galvoPorts';
import { selectIsLhy } from './lhyPorts';
import { selectIsRuida } from './ruidaPorts';

const MIN = 10;
const MAX = 200;

const KINDS: { kind: OverrideKind; label: I18nKey; hint: I18nKey }[] = [
  { kind: 'feed', label: 'machine.override.feed', hint: 'machine.override.feed.help' },
  { kind: 'spindle', label: 'machine.override.power', hint: 'machine.override.power.help' },
];

function OverrideRow({ kind, label, hint }: { kind: OverrideKind; label: I18nKey; hint: I18nKey }) {
  // `Ov:` solo llega de vez en cuando; mientras no se conoce se muestra 100 % (el valor de partida).
  const known = useStore((s) => s.status?.overrides?.[kind]);
  const value = known ?? 100;
  const whyKey = useStore(overrideBlockedReason);
  const why = whyKey ? t(whyKey) : undefined;
  const set = useStore((s) => s.setOverride);
  const bump = (delta: number) => void set(kind, Math.min(MAX, Math.max(MIN, value + delta)));

  return (
    <div className="mc-ovr" role="group" aria-label={t(label)}>
      <div className="mc-ovr__head">
        <span className="mc-ovr__label">{t(label)}</span>
        <span className="mc-ovr__value num" aria-live="polite" title={known === undefined ? t('machine.override.unknown') : undefined}>
          {known === undefined ? '—' : `${known} %`}
        </span>
      </div>
      <div className="mc-ovr__controls">
        {[-10, -1].map((d) => (
          <button key={d} type="button" className="btn" disabled={why !== undefined || value <= MIN} title={why ?? t('machine.override.step', { n: d })} aria-label={t('machine.override.step', { n: d })} onClick={() => bump(d)}>
            <Minus size={12} aria-hidden="true" />{Math.abs(d)}
          </button>
        ))}
        <NumberField label={t('machine.override.percent')} unit="%" value={value} min={MIN} max={MAX} step={10} disabled={why !== undefined} onCommit={(v) => void set(kind, v)} />
        {[1, 10].map((d) => (
          <button key={d} type="button" className="btn" disabled={why !== undefined || value >= MAX} title={why ?? t('machine.override.step', { n: `+${d}` })} aria-label={t('machine.override.step', { n: `+${d}` })} onClick={() => bump(d)}>
            <Plus size={12} aria-hidden="true" />{d}
          </button>
        ))}
        <button type="button" className="icon-btn icon-btn--boxed" disabled={why !== undefined || value === 100} title={why ?? t('machine.override.reset')} aria-label={t('machine.override.reset')} onClick={() => void set(kind, 100)}>
          <RotateCcw size={13} aria-hidden="true" />
        </button>
      </div>
      <p className="hint">{why ?? t(hint)}</p>
    </div>
  );
}

/**
 * Ajustes en vivo: velocidad y potencia en porcentaje con los overrides en tiempo real de GRBL.
 * Valen durante el trabajo y no cambian el G-code; el valor vigente es el `Ov:` que informa la
 * controladora. En Marlin y Smoothieware no existen y los controles quedan deshabilitados.
 */
export function OverridesSection() {
  // Ruida no tiene overrides en vivo: se muestra por qué en lugar de los controles.
  const ruida = useStore(selectIsRuida);
  // Tampoco la M2 Nano del K40: la potencia es la perilla de la máquina.
  const lhy = useStore(selectIsLhy);
  // Tampoco el galvo: la potencia sale de cada capa y para el láser solo hay Detener.
  const galvo = useStore(selectIsGalvo);
  if (galvo) {
    return (
      <PanelSection id="machine.overrides" title={t('machine.override')}>
        <GalvoNotApplicable reason="galvo.override.reason" />
      </PanelSection>
    );
  }
  if (lhy) {
    return (
      <PanelSection id="machine.overrides" title={t('machine.override')}>
        <LhyNotApplicable reason="lhymicro.override.reason" />
      </PanelSection>
    );
  }
  if (ruida) {
    return (
      <PanelSection id="machine.overrides" title={t('machine.override')}>
        <RuidaNotApplicable reason="ruida.override.reason" />
      </PanelSection>
    );
  }
  return (
    <PanelSection id="machine.overrides" title={t('machine.override')}>
      {KINDS.map((k) => (
        <OverrideRow key={k.kind} {...k} />
      ))}
    </PanelSection>
  );
}
