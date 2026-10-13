import type { ReactNode } from 'react';

/** Grupo de opciones excluyentes con botones tipo "chip" (radio accesible). */
export function ChoiceRow<T extends string | number | null>({
  legend,
  value,
  options,
  onChange,
}: {
  legend: string;
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (v: T) => void;
}) {
  return (
    <div className="fio-choice" role="radiogroup" aria-label={legend}>
      <span className="field__label">{legend}</span>
      <div className="fio-choice__row">
        {options.map((o) => (
          <button key={String(o.value)} type="button" role="radio" aria-checked={o.value === value} className="chip" onClick={() => onChange(o.value)}>
            {o.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export function CheckRow({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="fio-check">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  );
}

export function Hint({ children }: { children: ReactNode }) {
  return <p className="fio-hint">{children}</p>;
}
