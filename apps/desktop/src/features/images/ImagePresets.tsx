import { Save, Trash2 } from 'lucide-react';
import { useId, useState } from 'react';
import type { LayerSettings } from '@/api/types';
import { t } from '@/i18n';
import { useLayerSetters, type LayerTarget } from '@/features/layers/targets';
import { useStore } from '@/store';
import {
  applyValues, cleanName, FACTORY_PRESETS, loadUserPresets, matchPreset, MAX_NAME_LENGTH, newPresetId, saveUserPresets, valuesOf, withoutPreset, withPreset,
  type UserPreset,
} from './presets';

const CUSTOM = '';

/**
 * Presets de «Ajustar imagen»: elegir uno aplica brillo, contraste, gamma, realce y quitar motas de
 * una vez (un solo paso de deshacer); «Guardar como…» guarda los valores de ahora con un nombre.
 */
export function ImagePresets({ layer, target, disabled }: { layer: LayerSettings; target: LayerTarget; disabled: boolean }) {
  const id = useId();
  const { set } = useLayerSetters(target);
  const [user, setUser] = useState<UserPreset[]>(loadUserPresets);
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState('');
  const current = valuesOf(layer.image);
  const matched = matchPreset(current, user);
  const userMatch = user.find((p) => p.id === matched) ?? null;
  const factory = FACTORY_PRESETS.find((p) => p.id === matched) ?? null;

  const choose = (value: string) => {
    const values = FACTORY_PRESETS.find((p) => p.id === value)?.values ?? user.find((p) => p.id === value)?.values;
    if (values) set({ image: applyValues(layer.image, values) });
  };

  const commit = (list: UserPreset[]) => {
    setUser(list);
    saveUserPresets(list);
  };

  const save = () => {
    const next = withPreset(user, name, current, newPresetId);
    if (!next) {
      useStore.getState().notify('warning', t(cleanName(name) ? 'layers.preset.full' : 'layers.preset.noName'));
      return;
    }
    commit(next);
    setNaming(false);
    setName('');
  };

  return (
    <div className="lyr-field img-presets">
      <div className="lyr-field__top">
        <label htmlFor={id} className="lyr-field__label">{t('layers.preset')}</label>
        <select id={id} className="select lyr-select" value={matched ?? CUSTOM} disabled={disabled} onChange={(e) => choose(e.target.value)}>
          {matched === null && <option value={CUSTOM}>{t('layers.preset.custom')}</option>}
          <optgroup label={t('layers.preset.factory')}>
            {FACTORY_PRESETS.map((p) => <option key={p.id} value={p.id}>{t(p.name)}</option>)}
          </optgroup>
          {user.length > 0 && (
            <optgroup label={t('layers.preset.mine')}>
              {user.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </optgroup>
          )}
        </select>
      </div>
      {factory && <p className="lyr-help">{t(factory.hint)}</p>}
      {!naming ? (
        <div className="img-presets__row">
          <button type="button" className="btn" disabled={disabled} onClick={() => setNaming(true)}>
            <Save size={14} aria-hidden="true" />{t('layers.preset.save')}
          </button>
          {userMatch && (
            <button type="button" className="btn" disabled={disabled} onClick={() => commit(withoutPreset(user, userMatch.id))} title={t('layers.preset.delete.hint', { name: userMatch.name })}>
              <Trash2 size={14} aria-hidden="true" />{t('layers.preset.delete')}
            </button>
          )}
        </div>
      ) : (
        <form
          className="img-presets__row"
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          <input
            className="input"
            type="text"
            value={name}
            maxLength={MAX_NAME_LENGTH}
            placeholder={t('layers.preset.name')}
            aria-label={t('layers.preset.name')}
            // El campo aparece al pulsar «Guardar ajuste»: el foco va a lo que el usuario acaba de pedir.
            // eslint-disable-next-line jsx-a11y/no-autofocus
            autoFocus
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                e.stopPropagation();
                setNaming(false);
              }
            }}
          />
          <button type="submit" className="btn btn--accent">{t('layers.preset.saveOk')}</button>
          <button type="button" className="btn" onClick={() => setNaming(false)}>{t('layers.preset.cancel')}</button>
        </form>
      )}
    </div>
  );
}
