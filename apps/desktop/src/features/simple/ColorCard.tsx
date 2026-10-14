import { Flame, EyeOff, Image as ImageIcon, Save, RotateCcw, Scissors, SlidersHorizontal } from 'lucide-react';
import { useState } from 'react';
import type { LayerSettings } from '@/api/types';
import { mmMinToMmS, mmSToMmMin } from '@/features/layers/GalvoLayer';
import { NumField } from '@/features/layers/ParamFields';
import { minPowerApplies } from '@/features/layers/rules';
import { t, tn, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import {
  baseKnobs,
  chooseAction,
  lessBurn,
  moreCutPasses,
  resetToMaterial,
  saveAsMyMaterial,
  setDarkness,
  setNumber,
} from './actions';
import {
  actionBlock,
  DARK_STEPS,
  MAX_EASY_PASSES,
  knobChanges,
  knobsOf,
  thumbnail,
  type ColorGroup,
  type Knobs,
  type MaterialChoice,
  type SimpleAction,
} from './model';
import { useSimple } from './store';

const ACTIONS: { id: SimpleAction; icon: typeof Scissors; label: I18nKey }[] = [
  { id: 'cut', icon: Scissors, label: 'simple.action.cut' },
  { id: 'engrave', icon: Flame, label: 'simple.action.engrave' },
  { id: 'ignore', icon: EyeOff, label: 'simple.action.ignore' },
];

const DARK_LABEL: Record<number, I18nKey> = {
  [-3]: 'simple.dark.m3',
  [-2]: 'simple.dark.m2',
  [-1]: 'simple.dark.m1',
  0: 'simple.dark.0',
  1: 'simple.dark.p1',
  2: 'simple.dark.p2',
  3: 'simple.dark.p3',
};

export { actionBlock };

/** "Velocidad 300 mm/min" (o mm/s en una máquina galvo). */
export function speedText(speed: number, galvo: boolean): string {
  return galvo ? `${fmt(mmMinToMmS(speed), 1)} mm/s` : `${fmt(speed, 0)} mm/min`;
}

/** La muestra del color: el dibujo de sus formas, o un ícono de imagen. */
export function Swatch({ group, doc }: { group: ColorGroup; doc: Parameters<typeof thumbnail>[0] }) {
  const thumb = group.isImage ? null : thumbnail(doc, group.color);
  return (
    <span className="simple__swatch" style={{ borderColor: group.color }} aria-hidden="true">
      {thumb ? (
        <svg viewBox={thumb.viewBox} preserveAspectRatio="xMidYMid meet">
          <path d={thumb.d} fill="none" stroke={group.color} strokeWidth="2" vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
        </svg>
      ) : (
        <ImageIcon size={22} style={{ color: group.color }} />
      )}
    </span>
  );
}

export function Values({ layer, galvo }: { layer: LayerSettings; galvo: boolean }) {
  return (
    <p className="simple__values num" data-testid="values">
      {t('simple.values', { speed: speedText(layer.speed, galvo), power: fmt(layer.power, 0), passes: layer.passes })}
    </p>
  );
}

export function ChangeList({ base, cur, galvo }: { base: Knobs; cur: Knobs; galvo: boolean }) {
  const changes = knobChanges(base, cur);
  if (changes.length === 0) return null;
  return (
    <ul className="simple__changes" aria-label={t('simple.changed')}>
      {changes.map((c) => (
        <li key={c.field} className="num">
          {c.field === 'speed'
            ? t('simple.change.speed', { from: speedText(c.from, galvo), to: speedText(c.to, galvo) })
            : c.field === 'power'
              ? t('simple.change.power', { from: fmt(c.from, 1), to: fmt(c.to, 1) })
              : t('simple.change.passes', { from: c.from, to: c.to })}
        </li>
      ))}
    </ul>
  );
}

/** Los campos de velocidad, potencia y pasadas, con las mismas validaciones del editor de capas. */
function Numbers({ layer, galvo }: { layer: LayerSettings; galvo: boolean }) {
  return (
    <details className="simple__numbers">
      <summary>{t('simple.numbers')}</summary>
      {galvo ? (
        <NumField field="galvo_speed" label={t('galvo.layer.speed')} value={layer.speed} toView={mmMinToMmS} onApply={(v) => setNumber(layer.color, 'speed', mmSToMmMin(v))} />
      ) : (
        <NumField field="speed" label={t('layers.speed')} value={layer.speed} onApply={(v) => setNumber(layer.color, 'speed', v)} />
      )}
      <NumField
        field="power"
        label={layer.mode === 'image' ? t('layers.powerMax') : t('layers.power')}
        value={layer.power}
        onApply={(v) => setNumber(layer.color, 'power', v)}
        ctx={{ minPower: minPowerApplies(layer) ? layer.min_power : undefined }}
      />
      <NumField field="passes" label={t('layers.passes')} value={layer.passes} onApply={(v) => setNumber(layer.color, 'passes', v)} />
    </details>
  );
}

/** "Ajustar": claro / oscuro al grabar; más pasadas o menos quemado al cortar; los números; y guardar. */
function Adjust({ layer, action, galvo }: { layer: LayerSettings; action: 'cut' | 'engrave'; galvo: boolean }) {
  const tune = useSimple((s) => s.tune[layer.color.toLowerCase()]);
  const base = baseKnobs(layer.color);
  const cur = knobsOf(layer);
  const changed = base ? knobChanges(base, cur).length > 0 : false;
  const [saving, setSaving] = useState(false);
  const step = tune ?? 0;
  const maxed = layer.passes >= MAX_EASY_PASSES;
  return (
    <div className="simple__adjust">
      {action === 'engrave' ? (
        <div className="simple__darkness">
          <input
            type="range"
            className="simple__range"
            min={-DARK_STEPS}
            max={DARK_STEPS}
            step={1}
            value={step}
            aria-label={t('simple.dark.label')}
            aria-valuetext={tune === null ? t('simple.dark.custom') : t(DARK_LABEL[step] ?? 'simple.dark.0')}
            onChange={(e) => setDarkness(layer.color, Number(e.target.value))}
          />
          <div className="simple__range-ends" aria-hidden="true">
            <span>{t('simple.dark.lighter')}</span>
            <span>{t('simple.dark.darker')}</span>
          </div>
          <p className="simple__range-now" role="status">{tune === null ? t('simple.dark.custom') : t(DARK_LABEL[step] ?? 'simple.dark.0')}</p>
        </div>
      ) : (
        <div className="simple__row">
          <button
            type="button"
            className="btn simple__btn"
            disabled={maxed}
            title={maxed ? t('simple.cut.maxPasses', { n: MAX_EASY_PASSES }) : undefined}
            onClick={() => moreCutPasses(layer.color)}
          >
            {t('simple.cut.morePasses')}
          </button>
          <button type="button" className="btn simple__btn" onClick={() => lessBurn(layer.color)}>
            {t('simple.cut.lessBurn')}
          </button>
        </div>
      )}
      {action === 'cut' && maxed && <p className="hint">{t('simple.cut.maxPasses', { n: MAX_EASY_PASSES })}</p>}
      <Numbers layer={layer} galvo={galvo} />
      {base && <ChangeList base={base} cur={cur} galvo={galvo} />}
      {changed && (
        <div className="simple__row">
          <button type="button" className="btn simple__btn" onClick={() => resetToMaterial(layer.color)}>
            <RotateCcw size={15} aria-hidden="true" />
            {t('simple.reset')}
          </button>
          <button
            type="button"
            className="btn simple__btn"
            disabled={saving}
            onClick={() => {
              setSaving(true);
              void saveAsMyMaterial(layer.color).finally(() => setSaving(false));
            }}
          >
            <Save size={15} aria-hidden="true" />
            {t('simple.saveMine')}
          </button>
        </div>
      )}
    </div>
  );
}

/** Tarjeta de un color: su muestra, qué hacer con él, los valores que se usarán y el botón Ajustar. */
export function ColorCard({ group, choice }: { group: ColorGroup; choice: MaterialChoice | null }) {
  const doc = useStore((s) => s.doc);
  const galvo = useStore((s) => selectActiveDevice(s).kind === 'galvo');
  const action = useSimple((s) => s.actions[group.color.toLowerCase()]);
  const [open, setOpen] = useState(false);
  const layer = group.layer;
  const blocked = ACTIONS.map((a) => ({ ...a, why: actionBlock(a.id, group, choice) }));
  const adjustable = action === 'cut' || action === 'engrave';
  return (
    <li className="simple__color" data-color={group.color} data-action={action ?? 'none'}>
      <div className="simple__color-head">
        <Swatch group={group} doc={doc} />
        <div className="simple__color-name">
          <strong>{group.name || group.color}</strong>
          <span className="hint">{tn(group.isImage ? 'simple.color.images' : 'simple.color.shapes', group.count)}</span>
        </div>
      </div>
      <div className="simple__actions" role="group" aria-label={t('simple.actions.for', { name: group.name || group.color })}>
        {blocked.map(({ id, icon: Icon, label, why }) => (
          <button
            key={id}
            type="button"
            className="simple__action"
            data-action={id}
            aria-pressed={action === id}
            disabled={why !== null}
            title={why ? t(why) : undefined}
            aria-label={why ? `${t(label)}: ${t(why)}` : undefined}
            onClick={() => chooseAction(group.color, id)}
          >
            <Icon size={20} aria-hidden="true" />
            {t(label)}
          </button>
        ))}
      </div>
      {action === 'ignore' && <p className="hint">{t('simple.color.ignored')}</p>}
      {adjustable && (
        <>
          <Values layer={layer} galvo={galvo} />
          <button type="button" className="btn simple__btn" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
            <SlidersHorizontal size={15} aria-hidden="true" />
            {t('simple.adjust')}
          </button>
          {open && <Adjust layer={layer} action={action} galvo={galvo} />}
        </>
      )}
    </li>
  );
}
