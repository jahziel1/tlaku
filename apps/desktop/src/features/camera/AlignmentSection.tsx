import { useEffect, useMemo, useState } from 'react';
import { NumberField } from '@/ui/NumberField';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import type { CameraAlignment } from '@/store/camera';
import { applyAlignment } from './actions';
import { ALIGN_ORDER, rectCorners, type Quad, type Vec2 } from './geometry';
import { fitAlignment } from './setup';
import { correctedSpace } from './session';
import { CameraView } from './CameraView';
import { detectAt, pickPoint, useCenter } from './centerFinder';
import { addObservation } from './headCal';
import { useHeadAlignRef } from './headAlignState';
import { useHeadCal, useViewTool } from './headState';


type Draft = { px: (Vec2 | null)[]; mm: Vec2[] };

const PAPERS = { a4: [297, 210], letter: [279.4, 215.9] } as const;

function fromSaved(a: CameraAlignment | null, fallback: Quad): Draft {
  return a ? { px: a.image_px.map((p) => [...p] as Vec2), mm: a.bed_mm.map((p) => [...p] as Vec2) } : { px: [null, null, null, null], mm: fallback.map((p) => [...p] as Vec2) };
}

export interface AlignmentEditor {
  draft: Draft;
  active: number;
  setActive: (i: number) => void;
  setPoint: (i: number, p: Vec2) => void;
  place: (p: Vec2) => void;
  setMm: (i: number, axis: 0 | 1, v: number) => void;
  preset: (w: number, h: number) => void;
  clearPoints: () => void;
  note: string | null;
  setNote: (n: string | null) => void;
}

/** Estado del borrador de alineación: lo comparten la vista (puntos) y el formulario (mm). */
export function useAlignmentEditor(): AlignmentEditor {
  const saved = useStore((s) => s.cameraSetup.alignment);
  const device = useStore(selectActiveDevice);
  const bed: [number, number] = device?.bed ?? [300, 200];
  const [draft, setDraft] = useState<Draft>(() => fromSaved(saved, rectCorners(0, 0, bed[0], bed[1])));
  const [active, setActive] = useState(0);
  const [note, setNote] = useState<string | null>(null);

  // Si la alineación guardada cambia desde fuera (carga al iniciar, recalibración), se refleja.
  useEffect(() => {
    setDraft((d) => (saved ? fromSaved(saved, d.mm as Quad) : { ...d, px: [null, null, null, null] }));
  }, [saved]);

  const setPoint = (i: number, p: Vec2) => {
    setNote(null);
    setDraft((d) => ({ ...d, px: d.px.map((q, k) => (k === i ? p : q)) }));
  };
  return {
    draft,
    active,
    setActive,
    setPoint,
    place: (p) => {
      setPoint(active, p);
      const next = draft.px.findIndex((q, k) => k !== active && !q);
      setActive(next >= 0 ? next : active);
    },
    setMm: (i, axis, v) => {
      setNote(null);
      setDraft((d) => ({ ...d, mm: d.mm.map((q, k) => (k === i ? ((axis === 0 ? [v, q[1]] : [q[0], v]) as Vec2) : q)) }));
    },
    preset: (w, h) => setDraft((d) => ({ ...d, mm: rectCorners(0, 0, w, h) })),
    clearPoints: () => {
      setDraft((d) => ({ ...d, px: [null, null, null, null] }));
      setActive(0);
    },
    note,
    setNote,
  };
}

/**
 * Sección de la vista en vivo. Según la herramienta activa, los clics ponen los 4 puntos de la
 * alineación (arrastrables), registran la cruz de la calibración del cabezal o marcan el borde de
 * un objeto redondo para el buscador de centro.
 */
export function ViewSection({ editor }: { editor: AlignmentEditor }) {
  const status = useStore((s) => s.cameraStatus);
  const frozen = useStore((s) => s.cameraFrozen);
  useStore((s) => s.cameraSetup.calibration);
  useStore((s) => s.cameraStreamSize);
  const tool = useViewTool((s) => s.tool);
  const setTool = useViewTool((s) => s.setTool);
  const lastObs = useHeadCal((s) => s.obs.at(-1));
  const refPx = useHeadAlignRef((s) => s.px);
  const measuring = useHeadCal((s) => s.measuring);
  const picks = useCenter((s) => s.picks);
  const centerMode = useCenter((s) => s.mode);
  const live = status === 'live' || frozen;

  const align = tool === 'align';
  const points: (Vec2 | null)[] = align ? editor.draft.px : tool === 'headMark' ? [lastObs?.px ?? null] : tool === 'alignRef' ? [refPx] : picks.map((p) => p.px);
  const onPlace = align
    ? editor.place
    : tool === 'headMark'
      ? (p: Vec2) => {
          if (measuring) addObservation(p);
        }
      : tool === 'alignRef'
        ? (p: Vec2) => useHeadAlignRef.getState().setPx(p)
        : (p: Vec2) => (centerMode === 'auto' ? void detectAt(p) : pickPoint(p));
  const hint: I18nKey | null = align ? null : tool === 'headMark' ? 'camera.view.tool.headMark' : tool === 'alignRef' ? 'camera.headAlign.ref.tool' : centerMode === 'auto' ? 'camera.view.tool.centerAuto' : 'camera.view.tool.center';

  return (
    <section className="section">
      <CameraView
        space={correctedSpace()}
        points={points}
        active={align ? editor.active : -1}
        onPlace={onPlace}
        onMove={align ? editor.setPoint : undefined}
        onSelect={align ? editor.setActive : undefined}
        live={live}
      />
      {hint && (
        <p className="hint cam-tool" role="status">
          {t(hint)}{' '}
          <button type="button" className="chip" onClick={() => setTool('align')}>{t('camera.view.tool.back')}</button>
        </p>
      )}
    </section>
  );
}

/** Tabla de coordenadas, atajos de rellenado y botones de la alineación manual de 4 puntos. */
export function AlignmentSection({ editor }: { editor: AlignmentEditor }) {
  const saved = useStore((s) => s.cameraSetup.alignment);
  const device = useStore(selectActiveDevice);
  useStore((s) => s.cameraSetup.calibration);
  useStore((s) => s.cameraStreamSize);
  const bed: [number, number] = device?.bed ?? [300, 200];
  const { draft, active, setActive, setMm, preset, clearPoints, note, setNote } = editor;
  const space = correctedSpace();

  const placed = draft.px.filter(Boolean).length;
  const result = useMemo(() => {
    if (!space || placed < 4) return null;
    return fitAlignment(space, draft.px as Quad, draft.mm as Quad);
  }, [space, placed, draft]);
  const reason: I18nKey | null = !space ? 'camera.align.needCamera' : placed < 4 ? 'camera.align.needPoints' : !result ? 'camera.align.degenerate' : null;

  const apply = () => {
    if (!result) return;
    applyAlignment(result);
    setNote(t('camera.align.saved'));
  };

  return (
    <section className="section" aria-label={t('camera.align.title')}>
      <h3 className="panel-title">{t('camera.align.title')}</h3>
      <p className="hint">{t('camera.align.help')}</p>
      <ol className="cam-points">
        {ALIGN_ORDER.map((c, i) => (
          <li key={c} className="cam-point" data-active={active === i} data-set={!!draft.px[i]}>
            <button type="button" className="chip cam-point__pick" aria-pressed={active === i} onClick={() => setActive(i)} title={t('camera.align.activePoint', { n: i + 1 })}>
              <span className="num">{i + 1}</span>
              <span className="cam-point__name">{t(`camera.align.corner.${c}` as I18nKey)}</span>
            </button>
            <div className="field-grid cam-point__mm">
              <NumberField label={t('camera.align.x', { n: i + 1 })} unit="mm" value={draft.mm[i]![0]} step={1} onCommit={(v) => setMm(i, 0, v)} />
              <NumberField label={t('camera.align.y', { n: i + 1 })} unit="mm" value={draft.mm[i]![1]} step={1} onCommit={(v) => setMm(i, 1, v)} />
            </div>
          </li>
        ))}
      </ol>
      <div className="cam-wrap" role="group" aria-label={t('camera.align.presets')}>
        <span className="hint">{t('camera.align.presets')}:</span>
        <button type="button" className="chip" onClick={() => preset(bed[0], bed[1])}>{t('camera.align.preset.bed')}</button>
        <button type="button" className="chip" onClick={() => preset(...PAPERS.a4)}>{t('camera.align.preset.a4')}</button>
        <button type="button" className="chip" onClick={() => preset(...PAPERS.letter)}>{t('camera.align.preset.letter')}</button>
      </div>
      <button type="button" className="btn btn--accent" aria-disabled={reason !== null} title={reason ? t(reason) : undefined} onClick={reason ? undefined : apply}>
        {t('camera.align.apply')}
      </button>
      <div className="row cam-wrap">
        <button type="button" className="btn" disabled={placed === 0} onClick={clearPoints}>{t('camera.align.reset')}</button>
        <button type="button" className="btn" disabled={!saved} onClick={() => applyAlignment(null)}>{t('camera.align.clear')}</button>
      </div>
      {reason && <p className="hint">{t(reason)}</p>}
      <p className={saved ? 'hint cam-ok' : 'hint'} role="status">
        {note ?? (saved ? t('camera.align.done', { rms: fmt(saved.rms_mm, 3) }) : t('camera.align.none'))}
      </p>
    </section>
  );
}
