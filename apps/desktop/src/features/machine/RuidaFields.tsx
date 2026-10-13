import { TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import type { DeviceProfile } from '@/api/types';
import { t } from '@/i18n';
import { Field } from './DeviceFields';
import { RuidaExperimentalBadge } from './RuidaSection';
import { formatMagic, parseMagic, ruidaHost } from './ruidaPorts';

/**
 * Campos de un perfil Ruida (EXPERIMENTAL): IP de la controladora, magic (automático o a mano) y
 * si el trabajo se ejecuta al subirlo o solo se guarda como documento.
 */
export function RuidaFields({ draft, edit }: { draft: DeviceProfile; edit: (patch: Partial<DeviceProfile>) => void }) {
  const host = ruidaHost(draft);
  const magic = draft.ruida.magic;
  // Lo que se escribe puede no ser válido todavía: se guarda en el perfil solo cuando lo es.
  const [text, setText] = useState(magic === null ? '0x88' : formatMagic(magic));
  const typed = parseMagic(text);
  const setMagic = (value: number | null) => edit({ ruida: { ...draft.ruida, magic: value } });

  return (
    <>
      <p className="hint">
        <RuidaExperimentalBadge /> {t('ruida.experimental.note')}
      </p>
      <Field label={t('ruida.dev.host')} help="ruida.dev.host.help">
        {({ id, describedBy }) => (
          <input
            id={id}
            className="input num"
            inputMode="decimal"
            autoComplete="off"
            spellCheck={false}
            placeholder="192.168.1.100"
            value={draft.connection.type === 'udp' ? draft.connection.host : ''}
            aria-describedby={describedBy}
            onChange={(e) => edit({ connection: { type: 'udp', host: e.target.value } })}
          />
        )}
      </Field>
      {host === '' && <p className="hint">{t('ruida.dev.host.empty')}</p>}

      <div className="field-grid">
        <Field label={t('ruida.dev.magic')} help="ruida.dev.magic.help">
          {({ id, describedBy }) => (
            <select
              id={id}
              className="select"
              value={magic === null ? 'auto' : 'manual'}
              aria-describedby={describedBy}
              onChange={(e) => setMagic(e.target.value === 'auto' ? null : (typed ?? 0x88))}
            >
              <option value="auto">{t('ruida.dev.magic.auto')}</option>
              <option value="manual">{t('ruida.dev.magic.manual')}</option>
            </select>
          )}
        </Field>
        {magic !== null && (
          <Field label={t('ruida.dev.magicValue')} help="ruida.dev.magicValue.help">
            {({ id, describedBy }) => (
              <input
                id={id}
                className="input num"
                value={text}
                spellCheck={false}
                aria-invalid={typed === null || undefined}
                aria-describedby={describedBy}
                onChange={(e) => {
                  setText(e.target.value);
                  const n = parseMagic(e.target.value);
                  if (n !== null) setMagic(n);
                }}
              />
            )}
          </Field>
        )}
      </div>
      {magic !== null && typed === null && (
        <p className="hint hint--danger" role="alert">{t('ruida.dev.magic.bad')}</p>
      )}

      <Field label={t('ruida.dev.run')} help="ruida.dev.run.help">
        {({ id, describedBy }) => (
          <label className="mc-check">
            <input
              id={id}
              type="checkbox"
              checked={draft.ruida.run_after_upload}
              aria-describedby={describedBy}
              onChange={(e) => edit({ ruida: { ...draft.ruida, run_after_upload: e.target.checked } })}
            />
            <span>{t('ruida.dev.run.label')}</span>
          </label>
        )}
      </Field>
      {draft.ruida.run_after_upload && (
        <p className="warn-line" role="status">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('ruida.start.run')}</span>
        </p>
      )}

      <p className="hint"><strong>{t('ruida.dev.home')}.</strong> {t('ruida.dev.home.help')}</p>
      <p className="hint">{t('ruida.dev.rotary')}</p>
      <p className="hint">{t('ruida.dev.hidden')}</p>
    </>
  );
}
