import { Fragment, useEffect, useId, useMemo, useRef, useState } from 'react';
import type { FontFamily } from '@/api/types';
import { t } from '@/i18n';
import { filterFonts, isLineFont } from './fonts';

/** Máximo de familias que se dibujan a la vez (cada una carga su fuente); la búsqueda acota el resto. */
const MAX_OPTIONS = 80;

interface Props {
  /** Familia elegida (puede estar vacía o no instalada). */
  value: string;
  /** Familia que se usa si no hay una elegida. */
  fallback: string;
  families: FontFamily[];
  loading: boolean;
  onPick: (family: string) => void;
}

const cssFamily = (family: string) => (family ? `${JSON.stringify(family)}, system-ui, sans-serif` : undefined);

/** Selector de fuente con búsqueda; cada familia se muestra con su propia fuente. */
export function FontPicker({ value, fallback, families, loading, onPick }: Props) {
  const id = useId();
  const listId = `${id}-list`;
  const inputRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);

  const matches = useMemo(() => filterFonts(families, query), [families, query]);
  // El tope solo recorta las fuentes del sistema: las de una línea son pocas y no deben quedar fuera.
  const { shown, hidden } = useMemo(() => {
    const system = matches.filter((f) => !isLineFont(f));
    const line = matches.filter(isLineFont);
    return { shown: [...system.slice(0, MAX_OPTIONS), ...line], hidden: Math.max(0, system.length - MAX_OPTIONS) };
  }, [matches]);
  const firstLine = shown.findIndex(isLineFont);
  // Los encabezados solo hacen falta si hay de las dos clases a la vez.
  const sections = firstLine > 0;
  const shownValue = value.trim() || fallback;
  const isLineName = families.some((f) => isLineFont(f) && f.family.toLowerCase() === shownValue.toLowerCase());

  // Al abrir, la opción activa es la elegida (si está a la vista).
  useEffect(() => {
    if (!open) return;
    const at = shown.findIndex((f) => f.family.toLowerCase() === shownValue.toLowerCase());
    setActive(at >= 0 ? at : 0);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- solo al abrir o al cambiar la búsqueda
  }, [open, query]);

  useEffect(() => {
    if (open) document.getElementById(`${id}-opt-${active}`)?.scrollIntoView({ block: 'nearest' });
  }, [open, active, id]);

  const close = () => {
    setOpen(false);
    setQuery('');
  };
  const pick = (family: string) => {
    onPick(family);
    close();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    switch (e.key) {
      case 'ArrowDown':
      case 'ArrowUp': {
        e.preventDefault();
        if (!open) return setOpen(true);
        const n = shown.length;
        if (n) setActive((a) => (a + (e.key === 'ArrowDown' ? 1 : n - 1)) % n);
        break;
      }
      case 'Home':
      case 'End':
        if (open) {
          e.preventDefault();
          setActive(e.key === 'Home' ? 0 : Math.max(0, shown.length - 1));
        }
        break;
      case 'Enter':
        if (open) {
          e.preventDefault();
          const f = shown[active];
          if (f) pick(f.family);
        }
        break;
      case 'Escape':
        if (open) {
          e.preventDefault();
          e.stopPropagation();
          close();
        }
        break;
    }
  };

  const emptyText = loading ? t('text.font.loading') : families.length === 0 ? t('text.font.empty') : t('text.font.noMatch');

  return (
    <div className="fontpick">
      <input
        ref={inputRef}
        className="input fontpick__input"
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={open && shown[active] ? `${id}-opt-${active}` : undefined}
        aria-label={t('text.field.font')}
        placeholder={t('text.font.search')}
        autoComplete="off"
        spellCheck={false}
        value={open ? query : shownValue}
        style={open || isLineName ? undefined : { fontFamily: cssFamily(shownValue) }}
        onFocus={(e) => {
          setOpen(true);
          e.currentTarget.select();
        }}
        onClick={() => setOpen(true)}
        onBlur={close}
        onChange={(e) => {
          setQuery(e.target.value);
          setOpen(true);
        }}
        onKeyDown={onKeyDown}
      />
      {open && (
        <ul id={listId} role="listbox" className="fontpick__list" aria-label={t('text.field.font')}>
          {shown.length === 0 && <li className="fontpick__empty">{emptyText}</li>}
          {shown.map((f, i) => (
            <Fragment key={f.family}>
              {sections && i === 0 && (
                <li role="presentation" className="fontpick__section">{t('text.font.sectionSystem')}</li>
              )}
              {sections && i === firstLine && (
                <li role="presentation" className="fontpick__section">{t('text.font.sectionLine')}</li>
              )}
              <li
                id={`${id}-opt-${i}`}
                role="option"
                aria-selected={f.family.toLowerCase() === shownValue.toLowerCase()}
                data-active={i === active || undefined}
                data-line={isLineFont(f) || undefined}
                className="fontpick__opt"
                title={isLineFont(f) ? t('text.font.lineTitle') : t('text.font.styles', { styles: f.styles.join(', ') })}
                // Las de una línea se muestran con la interfaz: el navegador no tiene esa fuente.
                style={isLineFont(f) ? undefined : { fontFamily: cssFamily(f.family) }}
                // mousedown evita que el campo pierda el foco (y se cierre) antes del clic
                onMouseDown={(e) => e.preventDefault()}
                onMouseEnter={() => setActive(i)}
                onClick={() => pick(f.family)}
              >
                {f.family}
              </li>
            </Fragment>
          ))}
          {hidden > 0 && (
            <li className="fontpick__empty" aria-hidden="true">
              {t('text.font.more', { n: hidden })}
            </li>
          )}
        </ul>
      )}
    </div>
  );
}
