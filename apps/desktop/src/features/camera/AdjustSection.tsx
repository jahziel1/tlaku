import { RotateCcw } from 'lucide-react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { updateCameraPrefs } from './actions';
import { adjustOf, autoDpm, isAdjusted, MAX_OFFSET_MM, NO_ADJUST } from './adjust';
import { toBedAt } from './height';

/**
 * Ajuste manual del fondo (M35): resolución (puntos por mm) y desplazamientos X/Y para afinar la
 * alineación a mano. Solo con la cámara fija y una alineación guardada; la cámara del cabezal no lo usa.
 */
export function AdjustSection() {
  const alignment = useStore((s) => s.cameraSetup.alignment);
  const prefs = useStore((s) => s.cameraSetup.prefs);
  const headMode = useStore((s) => s.cameraSetup.head?.enabled === true);
  if (!alignment || headMode) return null;

  const adj = adjustOf(prefs);
  const auto = autoDpm(toBedAt(alignment, prefs.material_height_mm ?? 0), alignment.image_size);
  const manual = adj.dpm !== null;

  return (
    <section className="section" aria-label={t('camera.adjust.title')}>
      <h3 className="panel-title">{t('camera.adjust.title')}</h3>
      <p className="hint">{t('camera.adjust.help')}</p>
      <label className="mc-check">
        <input
          type="checkbox"
          checked={manual}
          onChange={(e) => updateCameraPrefs({ dpm: e.target.checked ? Math.round((auto || 1) * 1000) / 1000 : null })}
        />
        {t('camera.adjust.manualDpm')}
      </label>
      <NumberField
        label={t('camera.adjust.dpm')}
        unit={t('camera.adjust.dpmUnit')}
        value={adj.dpm ?? Math.round(auto * 1000) / 1000}
        min={0.01}
        max={1000}
        step={0.05}
        disabled={!manual}
        onCommit={(v) => updateCameraPrefs({ dpm: v })}
      />
      {auto > 0 && <p className="hint num">{t('camera.adjust.auto', { dpm: fmt(auto, 3) })}</p>}
      <div className="field-grid">
        <NumberField label={t('camera.adjust.x')} unit="mm" value={adj.dx} min={-MAX_OFFSET_MM} max={MAX_OFFSET_MM} step={0.1} onCommit={(v) => updateCameraPrefs({ offset_x_mm: v })} />
        <NumberField label={t('camera.adjust.y')} unit="mm" value={adj.dy} min={-MAX_OFFSET_MM} max={MAX_OFFSET_MM} step={0.1} onCommit={(v) => updateCameraPrefs({ offset_y_mm: v })} />
      </div>
      {isAdjusted(adj) && (
        <button type="button" className="btn" onClick={() => updateCameraPrefs({ dpm: NO_ADJUST.dpm, offset_x_mm: NO_ADJUST.dx, offset_y_mm: NO_ADJUST.dy })}>
          <RotateCcw size={14} aria-hidden="true" />
          {t('camera.adjust.reset')}
        </button>
      )}
    </section>
  );
}
