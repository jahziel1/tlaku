import { Crosshair, Move } from 'lucide-react';
import { useState } from 'react';
import type { CameraAlignment } from '@/api/types';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import type { Vec2 } from './geometry';
import { applyReferencePoint } from './headAlign';
import { openHeadAlignDialog, useHeadAlignRef } from './headAlignState';
import { useViewTool } from './headState';

/**
 * Alinear la cámara fija con el cabezal, sin grabar: explicación, botón del asistente y, con una
 * alineación guardada, el ajuste fino con un único punto de referencia conocido (un disparo de
 * prueba visible que se marca en la imagen).
 */
export function HeadAlignSection() {
  const alignment = useStore((s) => s.cameraSetup.alignment);
  const headMode = useStore((s) => s.cameraSetup.head?.enabled === true);
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const px = useHeadAlignRef((s) => s.px);
  const refBed = useHeadAlignRef((s) => s.bed);
  const marking = useHeadAlignRef((s) => s.marking);
  const setTool = useViewTool((s) => s.setTool);
  const [busy, setBusy] = useState(false);

  const apply = async () => {
    if (!px) return;
    setBusy(true);
    try {
      if ((await applyReferencePoint(px, refBed)) !== null) {
        useHeadAlignRef.getState().setPx(null);
        useHeadAlignRef.getState().setMarking(false);
        setTool('align');
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <HeadAlignView
      alignment={alignment}
      headMode={headMode}
      bed={bed}
      px={px}
      refBed={refBed}
      marking={marking}
      busy={busy}
      onOpen={openHeadAlignDialog}
      onRefBed={(b) => useHeadAlignRef.getState().setBed(b)}
      onMark={() => {
        useHeadAlignRef.getState().setMarking(!marking);
        setTool(marking ? 'align' : 'alignRef');
      }}
      onApply={() => void apply()}
    />
  );
}

export interface HeadAlignViewProps {
  alignment: CameraAlignment | null;
  headMode: boolean;
  bed: [number, number];
  px: Vec2 | null;
  refBed: Vec2;
  marking: boolean;
  busy: boolean;
  onOpen: () => void;
  onRefBed: (b: Vec2) => void;
  onMark: () => void;
  onApply: () => void;
}

export function HeadAlignView({ alignment, headMode, bed, px, refBed, marking, busy, onOpen, onRefBed, onMark, onApply }: HeadAlignViewProps) {
  const fromHead = alignment?.method === 'head_sweep';
  const loo = alignment?.samples?.[0]?.loo_rms_mm ?? null;
  return (
    <section className="section" aria-label={t('camera.headAlign.title')}>
      <h3 className="panel-title">{t('camera.headAlign.title')}</h3>
      <p className="hint">{t('camera.headAlign.help')}</p>
      <button type="button" className="btn btn--accent" aria-disabled={headMode} title={headMode ? t('camera.headAlign.reason.headMode') : undefined} onClick={() => !headMode && onOpen()}>
        <Move size={14} aria-hidden="true" />
        {t('camera.headAlign.open')}
      </button>
      {headMode && <p className="hint">{t('camera.headAlign.reason.headMode')}</p>}
      {fromHead && (
        <p className="hint cam-ok" role="status">
          {loo !== null ? t('camera.headAlign.current', { loo: fmt(loo, 2) }) : t('camera.headAlign.currentNoLoo')}
        </p>
      )}
      {alignment && (
        <details className="cam-ha__options">
          <summary>{t('camera.headAlign.ref.title')}</summary>
          <p className="hint">{t('camera.headAlign.ref.help')}</p>
          <div className="field-grid">
            <NumberField label={t('camera.headAlign.ref.x')} unit="mm" value={refBed[0]} min={0} max={bed[0]} step={0.5} onCommit={(v) => onRefBed([v, refBed[1]])} />
            <NumberField label={t('camera.headAlign.ref.y')} unit="mm" value={refBed[1]} min={0} max={bed[1]} step={0.5} onCommit={(v) => onRefBed([refBed[0], v])} />
          </div>
          <div className="row cam-wrap">
            <button type="button" className="btn" aria-pressed={marking} onClick={onMark}>
              <Crosshair size={14} aria-hidden="true" />
              {marking ? t('camera.headAlign.ref.marking') : t('camera.headAlign.ref.mark')}
            </button>
            <button type="button" className="btn btn--accent" aria-disabled={!px || busy} title={!px ? t('camera.headAlign.ref.needPoint') : undefined} onClick={() => px && !busy && onApply()}>
              {t('camera.headAlign.ref.apply')}
            </button>
          </div>
          <p className="hint num" role="status">{px ? t('camera.headAlign.ref.marked', { x: fmt(px[0], 1), y: fmt(px[1], 1) }) : t('camera.headAlign.ref.none')}</p>
        </details>
      )}
    </section>
  );
}
