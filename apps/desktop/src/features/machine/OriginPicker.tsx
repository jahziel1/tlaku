import { t, type I18nKey } from '@/i18n';

export const ANCHOR_KEYS = Array.from({ length: 9 }, (_, i) => `machine.anchor.${i}` as I18nKey);

interface Props {
  value: number;
  onChange: (index: number) => void;
  disabled?: boolean;
  /** Por qué está desactivado (se muestra en el tooltip). */
  reason?: string;
}

/**
 * Los 9 puntos del rectángulo del trabajo: el elegido es el que se ancla al cabezal o al origen de
 * usuario. Grupo de opciones con flechas del teclado (una sola parada de tabulación).
 */
export function OriginPicker({ value, onChange, disabled, reason }: Props) {
  const move = (from: number, key: string) => {
    const col = from % 3;
    const row = Math.floor(from / 3);
    const next =
      key === 'ArrowLeft' ? row * 3 + Math.max(0, col - 1)
      : key === 'ArrowRight' ? row * 3 + Math.min(2, col + 1)
      : key === 'ArrowUp' ? Math.max(0, row - 1) * 3 + col
      : key === 'ArrowDown' ? Math.min(2, row + 1) * 3 + col
      : null;
    return next;
  };
  return (
    <div className="mc-picker" role="radiogroup" aria-label={t('machine.anchor.label')} title={disabled ? reason : undefined} data-disabled={disabled || undefined}>
      {ANCHOR_KEYS.map((key, i) => (
        <button
          key={i}
          type="button"
          role="radio"
          aria-checked={value === i}
          aria-label={t(key)}
          title={disabled && reason ? reason : t(key)}
          tabIndex={value === i ? 0 : -1}
          disabled={disabled}
          className="mc-picker__dot"
          onClick={() => onChange(i)}
          onKeyDown={(e) => {
            const next = move(i, e.key);
            if (next === null) return;
            e.preventDefault();
            e.stopPropagation(); // las flechas no deben mover objetos del lienzo
            onChange(next);
            (e.currentTarget.parentElement?.children[next] as HTMLElement | undefined)?.focus();
          }}
        />
      ))}
    </div>
  );
}
