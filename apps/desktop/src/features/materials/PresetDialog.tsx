import { X } from 'lucide-react';
import { useEffect, useId, useMemo, useState, type FormEvent } from 'react';
import type { LayerSettings, MaterialPreset } from '@/api/types';
import { t, locale } from '@/i18n';
import { colorName } from '@/lib/palette';
import { selectActiveDevice, useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { activeLayerOf, savePresetFromActiveLayer, updatePreset } from './actions';
import { formProblem, settingsSummary, type PresetForm } from './library';
import { useMaterials } from './state';

/** Texto de un número para el campo de grosor (sin ceros sobrantes, con coma decimal). */
const thicknessText = (v: number | null) => (v === null ? '' : String(v).replace('.', ','));

function parseThickness(text: string): number | null {
  const s = text.trim();
  return s === '' ? null : Number(s.replace(',', '.'));
}

interface Props {
  /** Ajuste que se edita; `null` = guardar la capa activa como material nuevo. */
  preset: MaterialPreset | null;
  /** Capa activa del documento (de ella salen los ajustes de un material nuevo). */
  layer: LayerSettings | null;
  onClose: () => void;
}

/** Diálogo para guardar la capa activa como material o para editar los datos de uno. */
export function PresetDialog({ preset, layer, onClose }: Props) {
  const titleId = useId();
  const ids = { material: useId(), thickness: useId(), title: useId(), notes: useId(), device: useId(), list: useId() };
  const devices = useStore((s) => s.devices);
  const active = useStore(selectActiveDevice);
  const known = useMaterials((s) => s.library.presets);
  const materials = useMemo(() => [...new Set(known.map((p) => p.material))].sort((a, b) => a.localeCompare(b, locale())), [known]);

  const [material, setMaterial] = useState(preset?.material ?? '');
  const [thickness, setThickness] = useState(thicknessText(preset?.thickness ?? null));
  const [title, setTitle] = useState(preset?.title ?? '');
  const [notes, setNotes] = useState(preset?.notes ?? '');
  // Un material nuevo se vincula por defecto a la máquina con la que se probó.
  const [deviceId, setDeviceId] = useState<string | null>(preset ? preset.device_id : active.id);
  const [refresh, setRefresh] = useState(false);

  const form: PresetForm = { material, thickness: parseThickness(thickness), title, notes, device_id: deviceId };
  const problem = formProblem(form);
  const source = preset ? preset.settings : layer;
  const unknownDevice = deviceId !== null && !devices.some((d) => d.id === deviceId);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (problem) return;
    if (preset) updatePreset(preset.id, form, refresh && layer !== null);
    else savePresetFromActiveLayer(form);
    onClose();
  };

  return (
    <Modal labelId={titleId} onClose={onClose} portal closeOnBackdrop fallbackFocus="first" initialFocus="[data-autofocus]" backdropClassName="mat-backdrop" cardClassName="mat-modal">
      <form onSubmit={submit}>
        <header className="mat-modal__head">
          <div>
            <h2 id={titleId} className="mat-modal__title">{preset ? t('materials.editor.editTitle') : t('materials.editor.saveTitle')}</h2>
            {source && (
              <p className="hint num">
                {preset ? settingsSummary(source) : t('materials.editor.fromLayer', { name: layer?.name || colorName(layer?.color ?? '#000000'), summary: settingsSummary(source) })}
              </p>
            )}
          </div>
          <button type="button" className="icon-btn" title={t('materials.close')} aria-label={t('materials.close')} onClick={onClose}>
            <X size={16} aria-hidden="true" />
          </button>
        </header>

        <div className="mat-modal__body">
          <div className="field">
            <label className="field__label" htmlFor={ids.material}>{t('materials.field.material')}</label>
            <input id={ids.material} className="input" list={ids.list} value={material} placeholder={t('materials.field.material.placeholder')} data-autofocus onChange={(e) => setMaterial(e.target.value)} />
            <datalist id={ids.list}>{materials.map((m) => <option key={m} value={m} />)}</datalist>
          </div>
          <div className="field">
            <label className="field__label" htmlFor={ids.thickness}>{t('materials.field.thickness')}</label>
            <div className="field__box">
              <input id={ids.thickness} className="input field__input num" inputMode="decimal" value={thickness} placeholder={t('materials.field.thickness.placeholder')} onChange={(e) => setThickness(e.target.value)} />
              <span className="field__unit">mm</span>
            </div>
          </div>
          <div className="field">
            <label className="field__label" htmlFor={ids.title}>{t('materials.field.title')}</label>
            <input id={ids.title} className="input" value={title} placeholder={t('materials.field.title.placeholder')} onChange={(e) => setTitle(e.target.value)} />
          </div>
          <div className="field">
            <label className="field__label" htmlFor={ids.notes}>{t('materials.field.notes')}</label>
            <textarea id={ids.notes} className="input mat-notes" rows={3} value={notes} onChange={(e) => setNotes(e.target.value)} />
          </div>
          <div className="field">
            <label className="field__label" htmlFor={ids.device}>{t('materials.field.device')}</label>
            <select id={ids.device} className="select" value={deviceId ?? ''} onChange={(e) => setDeviceId(e.target.value || null)}>
              <option value="">{t('materials.field.device.any')}</option>
              {unknownDevice && <option value={deviceId}>{deviceId}</option>}
              {devices.map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
            </select>
          </div>
          {preset && (
            <label className="mat-check" title={layer ? undefined : t('materials.reason.noLayer')}>
              <input type="checkbox" checked={refresh && layer !== null} disabled={layer === null} onChange={(e) => setRefresh(e.target.checked)} />
              {layer ? t('materials.editor.refresh', { name: layer.name || colorName(layer.color) }) : t('materials.editor.refreshNone')}
            </label>
          )}
        </div>

        <footer className="mat-modal__foot">
          {problem && <p className="hint hint--danger mat-modal__problem" role="status">{t(problem)}</p>}
          <button type="button" className="btn" onClick={onClose}>{t('materials.cancel')}</button>
          <button type="submit" className="btn btn--accent" aria-disabled={problem !== null} title={problem ? t(problem) : undefined} onClick={(e) => problem && e.preventDefault()}>
            {t('materials.save')}
          </button>
        </footer>
      </form>
    </Modal>
  );
}

/** Aloja el diálogo según lo que pida `useMaterials().editor`. */
export function PresetDialogHost() {
  const editor = useMaterials((s) => s.editor);
  const close = useMaterials((s) => s.closeEditor);
  const preset = useMaterials((s) => (s.editor?.kind === 'edit' ? s.library.presets.find((p) => p.id === (s.editor as { id: string }).id) : undefined));
  const layer = useStore((s) => activeLayerOf(s));
  const valid = editor !== null && (editor.kind === 'edit' ? preset !== undefined : layer !== null);
  // Si el ajuste desaparece o la capa se borra mientras el diálogo está pedido, se cierra.
  useEffect(() => {
    if (editor && !valid) close();
  }, [editor, valid, close]);
  if (!editor || !valid) return null;
  if (editor.kind === 'edit') return <PresetDialog key={editor.id} preset={preset ?? null} layer={layer} onClose={close} />;
  return <PresetDialog key="new" preset={null} layer={layer} onClose={close} />;
}
