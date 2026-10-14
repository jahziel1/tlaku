import type { InputRequirement } from '@/bindings/InputRequirement';
import { t } from '@/i18n';

/** Qué se exige de una entrada de la tarjeta: nada, que esté alta o que esté baja. */
export type BitRule = 'any' | 'high' | 'low';

const BITS = 16;
const RULES: BitRule[] = ['any', 'high', 'low'];

/** El enclavamiento como una regla por entrada (0 a 15). */
export function rulesOf(req: InputRequirement | null | undefined): BitRule[] {
  return Array.from({ length: BITS }, (_, bit) => {
    if (!req || ((req.mask >> bit) & 1) === 0) return 'any';
    return (req.value >> bit) & 1 ? 'high' : 'low';
  });
}

/** Las reglas como `{ mask, value }`; `null` si ninguna entrada se exige. `value` solo trae bits de `mask`. */
export function requirementOf(rules: readonly BitRule[]): InputRequirement | null {
  let mask = 0;
  let value = 0;
  rules.forEach((rule, bit) => {
    if (rule === 'any') return;
    mask |= 1 << bit;
    if (rule === 'high') value |= 1 << bit;
  });
  return mask === 0 ? null : { mask, value };
}

/** `0x0008` con ceros a la izquierda, como el panel de las entradas. */
export const hex4 = (n: number): string => `0x${n.toString(16).toUpperCase().padStart(4, '0')}`;

/**
 * Editor del enclavamiento por entrada (M45): una regla por entrada de la tarjeta. Antes de cada trabajo y
 * de cada encuadre el motor lee las entradas y no sigue si no se cumple. Los bits del paro de emergencia y de
 * las alarmas de la fuente no están documentados: los fija quien integra la máquina, tras medirlos en el panel.
 */
export function GalvoInterlockEditor({ value, onChange }: { value: InputRequirement | null; onChange: (v: InputRequirement | null) => void }) {
  const rules = rulesOf(value);
  const set = (bit: number, rule: BitRule) => onChange(requirementOf(rules.map((r, i) => (i === bit ? rule : r))));
  return (
    <div className="mc-interlock">
      <div className="mc-interlock__grid" role="group" aria-label={t('galvo.dev.interlock')}>
        {rules.map((rule, bit) => (
          <label key={bit} className="mc-interlock__bit" data-rule={rule}>
            <span className="num">{t('galvo.dev.interlock.bit', { bit })}</span>
            <select className="select" value={rule} onChange={(e) => set(bit, e.target.value as BitRule)}>
              {RULES.map((r) => (
                <option key={r} value={r}>{t(`galvo.dev.interlock.${r}`)}</option>
              ))}
            </select>
          </label>
        ))}
      </div>
      <p className="hint num" role="status">
        {value ? t('galvo.dev.interlock.summary', { mask: hex4(value.mask), value: hex4(value.value) }) : t('galvo.dev.interlock.none')}
      </p>
      {value && (
        <button type="button" className="btn" onClick={() => onChange(null)}>{t('galvo.dev.interlock.clear')}</button>
      )}
    </div>
  );
}
