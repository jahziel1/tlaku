import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Crosshair, Minus, Plus, RotateCcw } from 'lucide-react';
import { useEffect, useId, useState, type KeyboardEvent } from 'react';
import type { GalvoFrameKind, GalvoState } from '@/api/types';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { stepFromKey } from '@/ui/stepKey';

/** Ajuste en vivo del encuadre: desplazamiento (mm) y escala respecto al tamaño real. */
export interface LiveAdjust {
  dx: number;
  dy: number;
  scale: number;
}

export const NO_ADJUST: LiveAdjust = { dx: 0, dy: 0, scale: 1 };

/** Formas de encuadre en el orden en que se ofrecen; el texto sale de `galvo.panel.frame.<id>`. */
export const FRAME_SHAPES: readonly GalvoFrameKind[] = ['rect', 'contour', 'hull', 'objects'];

/** Pasos (mm) con los que se mueve el encuadre. */
export const ADJUST_STEPS_MM = [0.1, 0.5, 1, 5] as const;

/** Cuánto crece o encoge cada pulsación de «más grande» o «más chico». */
export const SCALE_FACTOR = 1.1;

/** Mismos topes que el motor (`LightAdjust`): fuera de ellos el motor rechaza el ajuste. */
export const SCALE_RANGE = [0.05, 20] as const;
export const MAX_SHIFT_MM = 1000;

const round = (v: number, d = 4) => Math.round(v * 10 ** d) / 10 ** d;

/** Vueltas escritas por la persona: vacío = hasta detener; `'invalid'` si no es un entero de 1 a 9999. */
export function parseLoops(text: string): number | null | 'invalid' {
  const v = text.trim();
  if (v === '') return null;
  const n = Number(v);
  return Number.isInteger(n) && n >= 1 && n <= 9999 ? n : 'invalid';
}

/** El ajuste tras mover `(sx, sy)` pasos de `step` mm o multiplicar la escala por `factor`; `null` si se sale de los topes. */
export function nudge(cur: LiveAdjust, move: { sx?: number; sy?: number; factor?: number }, step: number): LiveAdjust | null {
  const next: LiveAdjust = {
    dx: round(cur.dx + (move.sx ?? 0) * step),
    dy: round(cur.dy + (move.sy ?? 0) * step),
    scale: round(cur.scale * (move.factor ?? 1)),
  };
  const ok = Math.abs(next.dx) <= MAX_SHIFT_MM && Math.abs(next.dy) <= MAX_SHIFT_MM && next.scale >= SCALE_RANGE[0] && next.scale <= SCALE_RANGE[1];
  return ok ? next : null;
}

export interface FrameControlsProps {
  state: Pick<GalvoState, 'framing'>;
  /** Vueltas del encuadre hasta ahora. */
  loops: number;
  /** Motivo por el que no se puede encuadrar ahora (texto ya traducido), o `null`. */
  blocked: string | null;
  /** El último ajuste en vivo se descartó por salirse del campo. */
  adjustRejected?: boolean;
  /** Encuadra la forma pedida; `loops` = vueltas tras las cuales termina solo (`null` = hasta detener). */
  onFrame: (shape: GalvoFrameKind, loops: number | null) => void;
  /** Mueve o escala el encuadre en curso. */
  onAdjust: (adjust: LiveAdjust) => void;
}

/**
 * Encuadre con el puntero rojo (M42): qué dibujar (rectángulo, trazos, casco convexo u objetos sueltos),
 * cuántas vueltas, el contador, y mover o escalar el encuadre mientras corre. El láser nunca se habilita.
 */
export function GalvoFrameControls({ state, loops, blocked, adjustRejected = false, onFrame, onAdjust }: FrameControlsProps) {
  const id = useId();
  const [shape, setShape] = useState<GalvoFrameKind>('rect');
  const [loopsText, setLoopsText] = useState('');
  const [requested, setRequested] = useState<number | null>(null);
  const [step, setStep] = useState<number>(1);
  const [adj, setAdj] = useState<LiveAdjust>(NO_ADJUST);
  const parsed = parseLoops(loopsText);
  const framing = state.framing;
  // Cada encuadre nuevo parte sin mover ni escalar.
  useEffect(() => {
    if (!framing) setAdj(NO_ADJUST);
  }, [framing]);
  const apply = (next: LiveAdjust | null) => {
    if (!next) return;
    setAdj(next);
    onAdjust(next);
  };
  const live = framing && !blocked;
  // ↑/↓: suman o restan repeticiones (entero, 1..9999); desde «∞» (vacío) parten de 1.
  const onLoopsKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const next = stepFromKey(e, typeof parsed === 'number' ? parsed : 1, 1, 1, 9999);
    if (next === null) return;
    e.preventDefault();
    setLoopsText(String(Math.round(next)));
  };
  return (
    <div className="mc-galvo-frame">
      <h4 className="mc-sub">{t('galvo.panel.frame')}</h4>
      <p className="hint">{t('galvo.panel.frame.help')}</p>
      <div className="field-grid">
        <div className="mc-field">
          <label htmlFor={`${id}-shape`} className="field__label">{t('galvo.panel.frame.shape')}</label>
          <select id={`${id}-shape`} className="select" value={shape} aria-describedby={`${id}-shape-help`} disabled={framing} onChange={(e) => setShape(e.target.value as GalvoFrameKind)}>
            {FRAME_SHAPES.map((k) => (
              <option key={k} value={k}>{t(`galvo.panel.frame.${k}`)}</option>
            ))}
          </select>
          <p id={`${id}-shape-help`} className="hint mc-help">{t(`galvo.panel.frame.${shape}.help`)}</p>
        </div>
        <div className="mc-field" data-invalid={parsed === 'invalid' || undefined}>
          <label htmlFor={`${id}-loops`} className="field__label">{t('galvo.panel.frame.loops')}</label>
          <input id={`${id}-loops`} className="input num" inputMode="numeric" value={loopsText} placeholder="∞" aria-invalid={parsed === 'invalid' || undefined} aria-describedby={`${id}-loops-help`} disabled={framing} onChange={(e) => setLoopsText(e.target.value)} onKeyDown={onLoopsKey} />
          <p id={`${id}-loops-help`} className="hint mc-help">{t('galvo.panel.frame.loops.help')}</p>
        </div>
      </div>
      <button
        type="button"
        className="btn btn--accent"
        disabled={blocked !== null || framing || parsed === 'invalid'}
        title={blocked ?? t(`galvo.panel.frame.${shape}.help`)}
        onClick={() => {
          if (parsed === 'invalid') return;
          setRequested(parsed);
          setAdj(NO_ADJUST);
          onFrame(shape, parsed);
        }}
      >
        <Crosshair size={15} aria-hidden="true" />{t('galvo.panel.frame.start')}
      </button>
      {framing && (
        <p className="hint num" role="status">
          {requested !== null ? t('galvo.panel.frame.counter', { n: loops, total: requested }) : t('galvo.panel.frame.counter.open', { n: loops })}
        </p>
      )}

      <div className="mc-galvo-adjust" role="group" aria-label={t('galvo.panel.adjust')}>
        <h5 className="mc-sub">{t('galvo.panel.adjust')}</h5>
        <p className="hint">{t('galvo.panel.adjust.help')}</p>
        {!framing && <p className="hint">{t('galvo.panel.adjust.only')}</p>}
        <div className="mc-field">
          <label htmlFor={`${id}-step`} className="field__label">{t('galvo.panel.adjust.step')}</label>
          <select id={`${id}-step`} className="select" value={step} onChange={(e) => setStep(Number(e.target.value))}>
            {ADJUST_STEPS_MM.map((s) => (
              <option key={s} value={s}>{`${fmt(s, 1)} mm`}</option>
            ))}
          </select>
        </div>
        <div className="mc-galvo-pad">
          <button type="button" className="btn" disabled={!live} aria-label={t('galvo.panel.adjust.left')} title={t('galvo.panel.adjust.left')} onClick={() => apply(nudge(adj, { sx: -1 }, step))}>
            <ArrowLeft size={15} aria-hidden="true" />
          </button>
          <button type="button" className="btn" disabled={!live} aria-label={t('galvo.panel.adjust.up')} title={t('galvo.panel.adjust.up')} onClick={() => apply(nudge(adj, { sy: 1 }, step))}>
            <ArrowUp size={15} aria-hidden="true" />
          </button>
          <button type="button" className="btn" disabled={!live} aria-label={t('galvo.panel.adjust.right')} title={t('galvo.panel.adjust.right')} onClick={() => apply(nudge(adj, { sx: 1 }, step))}>
            <ArrowRight size={15} aria-hidden="true" />
          </button>
          <button type="button" className="btn" disabled={!live} aria-label={t('galvo.panel.adjust.smaller')} title={t('galvo.panel.adjust.smaller')} onClick={() => apply(nudge(adj, { factor: 1 / SCALE_FACTOR }, step))}>
            <Minus size={15} aria-hidden="true" />
          </button>
          <button type="button" className="btn" disabled={!live} aria-label={t('galvo.panel.adjust.down')} title={t('galvo.panel.adjust.down')} onClick={() => apply(nudge(adj, { sy: -1 }, step))}>
            <ArrowDown size={15} aria-hidden="true" />
          </button>
          <button type="button" className="btn" disabled={!live} aria-label={t('galvo.panel.adjust.bigger')} title={t('galvo.panel.adjust.bigger')} onClick={() => apply(nudge(adj, { factor: SCALE_FACTOR }, step))}>
            <Plus size={15} aria-hidden="true" />
          </button>
        </div>
        <button type="button" className="btn" disabled={!live || (adj.dx === 0 && adj.dy === 0 && adj.scale === 1)} onClick={() => apply(NO_ADJUST)}>
          <RotateCcw size={14} aria-hidden="true" />{t('galvo.panel.adjust.reset')}
        </button>
        <p className="hint num">{t('galvo.panel.adjust.now', { dx: fmt(adj.dx, 2), dy: fmt(adj.dy, 2), scale: fmt(adj.scale, 2) })}</p>
        {adjustRejected && framing && (
          <p className="warn-line" role="alert">{t('galvo.panel.adjust.rejected')}</p>
        )}
      </div>
    </div>
  );
}
