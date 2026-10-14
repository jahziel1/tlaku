import { Crosshair, Plus } from 'lucide-react';
import { useMemo } from 'react';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { addRegistrationMarks, sanitizeCount, takeSelectionAsMarks } from './actions';
import type { ModeChoice } from './flow';
import { closestPair, designBox, MARK_COUNTS, marksMoved, MIN_MARK_DISTANCE_MM, type MarkKind } from './marks';
import { usePrintCut } from './printcutStore';

const KINDS: { id: MarkKind; label: I18nKey }[] = [
  { id: 'cross', label: 'printcut.marks.kind.cross' },
  { id: 'circle', label: 'printcut.marks.kind.circle' },
];

const MODES: { id: ModeChoice; label: I18nKey }[] = [
  { id: 'auto', label: 'printcut.mode.auto' },
  { id: 'rigid', label: 'printcut.mode.rigid' },
  { id: 'similarity', label: 'printcut.mode.similarity' },
  { id: 'affine', label: 'printcut.mode.affine' },
];

/** ¿Alguna marca elegida se movió o se borró? Sirve para avisar antes de seguir. */
export function useMarksMoved(): boolean {
  const doc = useStore((s) => s.doc);
  const marks = usePrintCut((s) => s.marks);
  return useMemo(() => marksMoved(doc, marks), [doc, marks]);
}

/** Por qué no se puede pasar al paso 2 (`null` si se puede). */
export function marksBlockedReason(count: number, spread: number, moved: boolean): I18nKey | null {
  if (count < 2) return 'printcut.marks.empty';
  if (spread < MIN_MARK_DISTANCE_MM) return 'printcut.marks.tooClose';
  if (moved) return 'printcut.marks.moved';
  return null;
}

/** Paso 1: marcas de registro del diseño, ya elegidas o por agregar. */
export function StepMarks() {
  const marks = usePrintCut((s) => s.marks);
  const kind = usePrintCut((s) => s.markKind);
  const count = usePrintCut((s) => s.markCount);
  const mode = usePrintCut((s) => s.mode);
  const patch = usePrintCut((s) => s.patch);
  const hasDesign = useStore((s) => designBox(s.doc) !== null);
  const moved = useMarksMoved();

  return (
    <>
      <p className="hint">{t('printcut.marks.intro')}</p>

      <section className="pc-section" aria-labelledby="pc-add">
        <h3 id="pc-add" className="pc-heading">{t('printcut.marks.add.title')}</h3>
        <p className="hint">{t('printcut.marks.add.help')}</p>
        <div className="pc-row" role="radiogroup" aria-label={t('printcut.marks.kind')}>
          {KINDS.map((k) => (
            <button key={k.id} type="button" role="radio" aria-checked={kind === k.id} className="chip" onClick={() => patch({ markKind: k.id })}>
              {t(k.label)}
            </button>
          ))}
        </div>
        <div className="pc-row" role="radiogroup" aria-label={t('printcut.marks.count')}>
          {MARK_COUNTS.map((n) => (
            <button key={n} type="button" role="radio" aria-checked={count === n} className="chip" onClick={() => patch({ markCount: sanitizeCount(n) })}>
              {t('printcut.marks.count.n', { n })}
            </button>
          ))}
        </div>
        <button
          type="button"
          className="btn"
          aria-disabled={!hasDesign}
          title={hasDesign ? undefined : t('printcut.marks.noDesign')}
          onClick={() => hasDesign && addRegistrationMarks(kind, count)}
        >
          <Plus size={14} aria-hidden="true" />{t('printcut.marks.add')}
        </button>
      </section>

      <section className="pc-section" aria-labelledby="pc-use">
        <h3 id="pc-use" className="pc-heading">{t('printcut.marks.use.title')}</h3>
        <p className="hint">{t('printcut.marks.use.help')}</p>
        <button type="button" className="btn" onClick={() => takeSelectionAsMarks()}>
          <Crosshair size={14} aria-hidden="true" />{t('printcut.marks.use')}
        </button>
        {marks.length === 0 ? (
          <p className="hint">{t('printcut.marks.empty')}</p>
        ) : (
          <ol className="pc-list" aria-label={t('printcut.marks.list')}>
            {marks.map((m, i) => (
              <li key={m.ids.join(',')} className="num">
                {t('printcut.marks.item', { n: i + 1, x: fmt(m.point[0], 2), y: fmt(m.point[1], 2) })}
              </li>
            ))}
          </ol>
        )}
        {moved && marks.length > 0 && <p className="warn-line" role="alert">{t('printcut.marks.moved')}</p>}
        {marks.length >= 2 && closestPair(marks.map((m) => m.point)) < MIN_MARK_DISTANCE_MM && (
          <p className="warn-line" role="alert">{t('printcut.marks.tooClose', { min: MIN_MARK_DISTANCE_MM })}</p>
        )}
      </section>

      <section className="pc-section" aria-labelledby="pc-mode">
        <h3 id="pc-mode" className="pc-heading">{t('printcut.mode')}</h3>
        <div className="pc-row" role="radiogroup" aria-label={t('printcut.mode')}>
          {MODES.map((m) => (
            <button key={m.id} type="button" role="radio" aria-checked={mode === m.id} className="chip" onClick={() => patch({ mode: m.id })}>
              {t(m.label)}
            </button>
          ))}
        </div>
        <p className="hint">{t('printcut.mode.help')}</p>
      </section>
    </>
  );
}
