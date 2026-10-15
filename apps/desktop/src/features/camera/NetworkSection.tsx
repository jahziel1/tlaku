import { PanelSection } from '@/ui/PanelSection';
import { useState } from 'react';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { updateCameraPrefs } from './actions';
import { hasCredentials, looksLocal, maskCredentials, parseNetworkUrl, type UrlProblem } from './networkUrl';
import { checkPresetName, findPreset, MAX_PRESETS, applyPreset, deletePreset, matchesPreset, savePreset } from './presets';
import { startCamera } from './session';

/** Reabre la cámara con la fuente nueva, si estaba encendida. */
function reopenIfOn(): void {
  if (useStore.getState().cameraStatus !== 'off') void startCamera();
}

/** Elige entre la cámara local y una de red. */
export function SourceKindField() {
  const source = useStore((s) => s.cameraSetup.prefs.source ?? 'local');
  return (
    <div className="field">
      <label className="field__label" htmlFor="cam-kind">{t('camera.source.kind')}</label>
      <select
        id="cam-kind"
        className="select"
        value={source}
        onChange={(e) => {
          updateCameraPrefs({ source: e.target.value === 'network' ? 'network' : 'local' });
          reopenIfOn();
        }}
      >
        <option value="local">{t('camera.source.kind.local')}</option>
        <option value="network">{t('camera.source.kind.network')}</option>
      </select>
    </div>
  );
}

/**
 * Dirección de la cámara de red. Mientras no se edita se muestra con la clave oculta; al enfocar
 * aparece la URL completa. Se guarda en este equipo (`camera.json`) y nunca va en un paquete.
 */
export function NetworkUrlField() {
  const saved = useStore((s) => s.cameraSetup.prefs.network_url ?? '');
  const [draft, setDraft] = useState<string | null>(null);
  const [problem, setProblem] = useState<UrlProblem | null>(null);

  const commit = () => {
    if (draft === null) return;
    const text = draft.trim();
    if (text === '') {
      updateCameraPrefs({ network_url: null });
    } else {
      const parsed = parseNetworkUrl(text);
      if (!parsed.ok) return setProblem(parsed.reason);
      if (parsed.url !== saved) {
        updateCameraPrefs({ network_url: parsed.url });
        reopenIfOn();
      }
    }
    setProblem(null);
    setDraft(null);
  };

  return (
    <div className="field">
      <label className="field__label" htmlFor="cam-url">{t('camera.network.url')}</label>
      <input
        id="cam-url"
        className="field__input num cam-url"
        type="text"
        inputMode="url"
        autoComplete="off"
        spellCheck={false}
        placeholder="http://192.168.1.20/snapshot.jpg"
        value={draft ?? maskCredentials(saved)}
        aria-invalid={problem !== null}
        onFocus={() => setDraft(saved)}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') commit();
          else if (e.key === 'Escape') {
            setDraft(null);
            setProblem(null);
          }
        }}
      />
      {problem && <p className="hint hint--danger" role="alert">{t(`camera.network.err.${problem}` as I18nKey)}</p>}
      <p className="hint">{t('camera.network.help')}</p>
      {saved !== '' && !looksLocal(saved) && <p className="warn-line" role="alert">{t('camera.network.notLocal')}</p>}
      {hasCredentials(saved) && <p className="hint">{t('camera.network.credentials')}</p>}
    </div>
  );
}

const NO_PRESETS: never[] = [];

/** Configuraciones de cámara con nombre: guardar la actual y volver a una guardada. */
export function PresetsSection() {
  // Un `[]` nuevo en cada lectura haría que el selector nunca se estabilice (bucle de renderizado).
  const presets = useStore((s) => s.cameraSetup.presets ?? NO_PRESETS);
  const prefs = useStore((s) => s.cameraSetup.prefs);
  const [name, setName] = useState('');
  const [selected, setSelected] = useState('');
  const [problem, setProblem] = useState<'empty' | 'long' | 'full' | null>(null);
  const current = presets.find((p) => matchesPreset(p, prefs))?.name ?? '';
  const chosen = findPreset(presets, selected) ? selected : current;

  const save = () => {
    const bad = savePreset(name);
    setProblem(bad);
    if (!bad) {
      setSelected(name.trim());
      setName('');
    }
  };

  return (
    <PanelSection id="camera.presets" title={t('camera.presets.title')}>
      <p className="hint">{t('camera.presets.help')}</p>
      {presets.length > 0 ? (
        <div className="field">
          <label className="field__label" htmlFor="cam-preset">{t('camera.presets.choose')}</label>
          <select
            id="cam-preset"
            className="select"
            value={chosen}
            onChange={(e) => {
              setSelected(e.target.value);
              if (e.target.value) applyPreset(e.target.value);
            }}
          >
            <option value="">{t('camera.presets.none')}</option>
            {presets.map((p) => <option key={p.name} value={p.name}>{p.name}</option>)}
          </select>
        </div>
      ) : (
        <p className="hint">{t('camera.presets.empty')}</p>
      )}
      <div className="row cam-wrap">
        <input
          className="field__input grow"
          type="text"
          aria-label={t('camera.presets.name')}
          placeholder={t('camera.presets.name')}
          value={name}
          maxLength={80}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && save()}
        />
        <button type="button" className="btn" aria-disabled={checkPresetName(name) !== null} onClick={() => checkPresetName(name) === null && save()}>
          {t('camera.presets.save')}
        </button>
        {chosen !== '' && (
          <button type="button" className="btn" onClick={() => deletePreset(chosen)}>{t('camera.presets.delete')}</button>
        )}
      </div>
      {problem && <p className="hint hint--danger" role="alert">{t(`camera.presets.err.${problem}` as I18nKey, { max: MAX_PRESETS })}</p>}
    </PanelSection>
  );
}
