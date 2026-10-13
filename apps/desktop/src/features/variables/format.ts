/** Formato de números de serie y de fechas y horas para las variables de texto. */

export type Lang = 'es' | 'en';

export const DEFAULT_DATE_FORMAT = 'dd/MM/yyyy';
export const DEFAULT_TIME_FORMAT = 'HH:mm';

const MONTHS: Record<Lang, string[]> = {
  es: ['enero', 'febrero', 'marzo', 'abril', 'mayo', 'junio', 'julio', 'agosto', 'septiembre', 'octubre', 'noviembre', 'diciembre'],
  en: ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'],
};
const MONTHS_SHORT: Record<Lang, string[]> = {
  es: ['ene', 'feb', 'mar', 'abr', 'may', 'jun', 'jul', 'ago', 'sep', 'oct', 'nov', 'dic'],
  en: ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'],
};
/** Desde el domingo (como `Date.getDay`). */
const DAYS: Record<Lang, string[]> = {
  es: ['domingo', 'lunes', 'martes', 'miércoles', 'jueves', 'viernes', 'sábado'],
  en: ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'],
};
const DAYS_SHORT: Record<Lang, string[]> = {
  es: ['dom', 'lun', 'mar', 'mié', 'jue', 'vie', 'sáb'],
  en: ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'],
};

const pad = (n: number, width: number) => String(n).padStart(width, '0');

/** Número de serie con ceros a la izquierda; `hex`/`HEX` lo escribe en hexadecimal. */
export function formatSerial(value: number, digits: number, hex: false | 'lower' | 'upper' = false): string {
  const n = Math.trunc(value);
  const body = hex ? Math.abs(n).toString(16) : String(Math.abs(n));
  const cased = hex === 'upper' ? body.toUpperCase() : body;
  return (n < 0 ? '-' : '') + cased.padStart(Math.max(0, digits), '0');
}

/** Opción de `{serie:...}`: `6`, `hex`, `HEX`, `hex4`... `null` si no se entiende. */
export function parseSerialOption(arg: string): { digits: number | null; hex: false | 'lower' | 'upper' } | null {
  const a = arg.trim();
  if (a === '') return { digits: null, hex: false };
  const m = /^(hex|HEX)?(\d{1,2})?$/.exec(a);
  if (!m) return null;
  const hex = m[1] === undefined ? false : m[1] === 'HEX' ? 'upper' : 'lower';
  return { digits: m[2] === undefined ? null : Number(m[2]), hex };
}

type Piece = { kind: 'lit'; text: string } | { kind: 'tok'; letter: string; len: number };

/** Parte el formato en literales (entre comillas simples o caracteres sueltos) y marcas. */
function pieces(format: string): Piece[] {
  const out: Piece[] = [];
  let i = 0;
  while (i < format.length) {
    const c = format[i]!;
    if (c === "'") {
      let j = i + 1;
      let text = '';
      while (j < format.length) {
        if (format[j] === "'") {
          if (format[j + 1] === "'") {
            text += "'";
            j += 2;
            continue;
          }
          break;
        }
        text += format[j];
        j += 1;
      }
      // Dos comillas seguidas fuera de un literal son una comilla.
      out.push({ kind: 'lit', text: text === '' && j === i + 1 ? "'" : text });
      i = j + 1;
    } else if (c === 'A' && format[i + 1] === 'P') {
      out.push({ kind: 'tok', letter: 'AP', len: 2 });
      i += 2;
    } else if (c === 'a' && format[i + 1] === 'p') {
      out.push({ kind: 'tok', letter: 'ap', len: 2 });
      i += 2;
    } else if ('dMyaHhms'.includes(c)) {
      let j = i;
      while (format[j] === c) j += 1;
      // `a` (año) se trata como `y`.
      out.push({ kind: 'tok', letter: c === 'a' ? 'y' : c, len: j - i });
      i = j;
    } else {
      out.push({ kind: 'lit', text: c });
      i += 1;
    }
  }
  return out;
}

/**
 * Fecha y hora con un formato (ver `syntax.ts`). La `m` minúscula es de minutos si está junto a
 * una hora o a segundos (`HH:mm`, `mm:ss`) y de mes en los demás casos (`dd/mm/aaaa`).
 */
export function formatDate(date: Date, format: string, lang: Lang): string {
  const ps = pieces(format);
  const isTok = (p: Piece | undefined, letters: string): p is Extract<Piece, { kind: 'tok' }> =>
    p?.kind === 'tok' && letters.includes(p.letter) && p.letter.length === 1;
  /** Marca más cercana hacia un lado, saltando los separadores sueltos. */
  const near = (from: number, dir: 1 | -1): Piece | undefined => {
    for (let k = from + dir; k >= 0 && k < ps.length; k += dir) {
      const p = ps[k]!;
      if (p.kind === 'tok') return p;
      if (/[\p{L}\p{N}]/u.test(p.text)) return undefined;
    }
    return undefined;
  };
  const h24 = date.getHours();
  return ps
    .map((p, idx) => {
      if (p.kind === 'lit') return p.text;
      const { letter, len } = p;
      switch (letter) {
        case 'd':
          return len >= 4 ? DAYS[lang][date.getDay()]! : len === 3 ? DAYS_SHORT[lang][date.getDay()]! : pad(date.getDate(), len === 2 ? 2 : 1);
        case 'M':
          return monthPiece(date, len, lang);
        case 'y':
          return len <= 2 ? pad(date.getFullYear() % 100, 2) : pad(date.getFullYear(), 4);
        case 'H':
          return pad(h24, len >= 2 ? 2 : 1);
        case 'h':
          return pad(h24 % 12 === 0 ? 12 : h24 % 12, len >= 2 ? 2 : 1);
        case 'm': {
          const minutes = isTok(near(idx, -1), 'Hh') || isTok(near(idx, 1), 's');
          return minutes ? pad(date.getMinutes(), len >= 2 ? 2 : 1) : monthPiece(date, Math.min(len, 2), lang);
        }
        case 's':
          return pad(date.getSeconds(), len >= 2 ? 2 : 1);
        case 'AP':
          return h24 < 12 ? 'AM' : 'PM';
        case 'ap':
          return h24 < 12 ? 'am' : 'pm';
        default:
          return '';
      }
    })
    .join('');
}

function monthPiece(date: Date, len: number, lang: Lang): string {
  const m = date.getMonth();
  if (len >= 4) return MONTHS[lang][m]!;
  if (len === 3) return MONTHS_SHORT[lang][m]!;
  return pad(m + 1, len === 2 ? 2 : 1);
}
