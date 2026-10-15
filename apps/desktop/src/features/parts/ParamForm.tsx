import { useId, type ReactNode } from 'react';
import type { GeneratorInfo } from '@/bindings/GeneratorInfo';
import type { ParamSpec } from '@/bindings/ParamSpec';
import type { PartsError } from '@/bindings/PartsError';
import { t, type I18nKey } from '@/i18n';
import { LengthField } from '@/ui/LengthField';
import { NumberField } from '@/ui/NumberField';
import type { FormValues } from './partsStore';

/** Texto de un campo o de una opción: las claves salen del id del generador y del campo (`parts.box.width_mm`). */
const pt = (key: string) => t(`parts.${key}` as I18nKey);

function visible(spec: ParamSpec, values: FormValues): boolean {
  const c = spec.when;
  return !c || c.any_of.includes(String(values[c.key]));
}

function Choice({ id, spec, generator, value, onChange }: { id: string; spec: ParamSpec; generator: string; value: string; onChange: (v: string) => void }) {
  const fieldId = useId();
  if (spec.kind.type !== 'choice') return null;
  return (
    <div className="field">
      <label htmlFor={fieldId} className="field__label">{pt(`${id}.${spec.key}`)}</label>
      <select id={fieldId} className="select" value={value} onChange={(e) => onChange(e.target.value)}>
        {spec.kind.options.map((o) => (
          <option key={o} value={o}>{pt(`${generator}.${spec.key}.${o}`)}</option>
        ))}
      </select>
    </div>
  );
}

function Field({ info, spec, values, error, onChange }: { info: GeneratorInfo; spec: ParamSpec; values: FormValues; error: PartsError | null; onChange: (key: string, value: unknown) => void }) {
  const label = pt(`${info.id}.${spec.key}`);
  const raw = values[spec.key];
  const kind = spec.kind;
  const errId = useId();
  let control: ReactNode;
  if (kind.type === 'length') {
    control = <LengthField label={label} value={typeof raw === 'number' ? raw : kind.default} min={kind.min} max={kind.max} step={kind.step} onCommit={(v) => onChange(spec.key, v)} />;
  } else if (kind.type === 'number') {
    control = <NumberField label={label} value={typeof raw === 'number' ? raw : kind.default} min={kind.min} max={kind.max} step={kind.step} onCommit={(v) => onChange(spec.key, v)} />;
  } else if (kind.type === 'int') {
    control = <NumberField label={label} value={typeof raw === 'number' ? raw : kind.default} min={kind.min} max={kind.max} step={1} format={(v) => String(Math.round(v))} onCommit={(v) => onChange(spec.key, Math.round(v))} />;
  } else if (kind.type === 'bool') {
    control = (
      <label className="parts__check">
        <input type="checkbox" checked={typeof raw === 'boolean' ? raw : kind.default} onChange={(e) => onChange(spec.key, e.target.checked)} />
        {label}
      </label>
    );
  } else {
    control = <Choice id={info.id} spec={spec} generator={info.id} value={typeof raw === 'string' ? raw : kind.default} onChange={(v) => onChange(spec.key, v)} />;
  }
  const mine = error?.field === spec.key ? error : null;
  return (
    <div className="parts__field" data-invalid={mine ? '' : undefined} aria-describedby={mine ? errId : undefined}>
      {control}
      {mine && <p id={errId} className="hint hint--danger parts__error" role="alert">{mine.message}</p>}
    </div>
  );
}

/** Formulario de un generador, generado a partir de su lista de campos (`ParamSpec`). */
export function ParamForm({ info, values, error, onChange }: { info: GeneratorInfo; values: FormValues; error: PartsError | null; onChange: (key: string, value: unknown) => void }) {
  const sections: { name: string; specs: ParamSpec[] }[] = [];
  for (const spec of info.params) {
    if (!visible(spec, values)) continue;
    const last = sections[sections.length - 1];
    if (last && last.name === spec.section) last.specs.push(spec);
    else sections.push({ name: spec.section, specs: [spec] });
  }
  return (
    <div className="parts__form">
      {sections.map((sec) => (
        <fieldset key={sec.name} className="parts__group">
          <legend className="mc-sub">{t(`parts.section.${sec.name}` as I18nKey)}</legend>
          <div className="parts__fields">
            {sec.specs.map((spec) => (
              <Field key={spec.key} info={info} spec={spec} values={values} error={error} onChange={onChange} />
            ))}
          </div>
        </fieldset>
      ))}
    </div>
  );
}
