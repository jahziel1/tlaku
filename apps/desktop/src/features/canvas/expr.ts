/**
 * Evaluador de expresiones numéricas para la barra de edición (`100/3`, `(10+2)*4`, `5in`, `50%`).
 * No usa `eval`: es un analizador descendente recursivo con una gramática pequeña.
 *
 *   expr   := term (('+' | '-') term)*
 *   term   := power (('*' | '/') power)*
 *   power  := unary ('^' power)?
 *   unary  := ('+' | '-') unary | atom
 *   atom   := (número | constante | función '(' expr ')' | '(' expr ')') unidad? '%'?
 */

export type ExprMode = 'length' | 'angle';

const LENGTH_UNITS: Record<string, number> = { mm: 1, cm: 10, m: 1000, in: 25.4, '"': 25.4 };
const ANGLE_UNITS: Record<string, number> = { deg: 1, '°': 1, rad: 180 / Math.PI };
const CONSTANTS: Record<string, number> = { pi: Math.PI };
const FUNCTIONS: Record<string, (x: number) => number> = {
  sqrt: Math.sqrt, abs: Math.abs, sin: Math.sin, cos: Math.cos, tan: Math.tan,
};

interface Ctx {
  /** Valor actual del campo, para `%`. */
  current: number;
  mode: ExprMode;
}

class Parser {
  private i = 0;
  constructor(private readonly src: string, private readonly ctx: Ctx) {}

  parse(): number {
    const v = this.expr();
    this.ws();
    if (this.i < this.src.length) throw new Error('sobra texto');
    return v;
  }

  private ws(): void {
    while (this.i < this.src.length && /\s/.test(this.src[this.i])) this.i++;
  }

  private peek(): string {
    this.ws();
    return this.src[this.i] ?? '';
  }

  private expr(): number {
    let v = this.term();
    for (;;) {
      const c = this.peek();
      if (c === '+' || c === '-') {
        this.i++;
        const r = this.term();
        v = c === '+' ? v + r : v - r;
      } else return v;
    }
  }

  private term(): number {
    let v = this.power();
    for (;;) {
      const c = this.peek();
      if (c === '*' || c === '×' || c === '/') {
        this.i++;
        const r = this.power();
        if (c !== '/') v *= r;
        else if (r === 0) throw new Error('división entre cero');
        else v /= r;
      } else return v;
    }
  }

  private power(): number {
    const base = this.unary();
    if (this.peek() === '^') {
      this.i++;
      return Math.pow(base, this.power());
    }
    return base;
  }

  private unary(): number {
    const c = this.peek();
    if (c === '-') {
      this.i++;
      return -this.unary();
    }
    if (c === '+') {
      this.i++;
      return this.unary();
    }
    return this.atom();
  }

  private atom(): number {
    const c = this.peek();
    let v: number;
    if (c === '(') {
      this.i++;
      v = this.expr();
      if (this.peek() !== ')') throw new Error('falta )');
      this.i++;
    } else if (/[0-9.,]/.test(c)) {
      v = this.number();
    } else if (/[a-zA-Z]/.test(c)) {
      const name = this.word().toLowerCase();
      if (name in CONSTANTS) v = CONSTANTS[name];
      else if (name in FUNCTIONS) {
        if (this.peek() !== '(') throw new Error('falta (');
        this.i++;
        const arg = this.expr();
        if (this.peek() !== ')') throw new Error('falta )');
        this.i++;
        v = FUNCTIONS[name](arg);
      } else throw new Error(`desconocido: ${name}`);
    } else throw new Error('se esperaba un número');
    return this.suffix(v);
  }

  private number(): number {
    const m = /^(\d+([.,]\d*)?|[.,]\d+)([eE][+-]?\d+)?/.exec(this.src.slice(this.i));
    if (!m) throw new Error('número no válido');
    this.i += m[0].length;
    return Number(m[0].replace(',', '.'));
  }

  private word(): string {
    const m = /^[a-zA-Z]+/.exec(this.src.slice(this.i));
    this.i += m ? m[0].length : 0;
    return m ? m[0] : '';
  }

  /** Unidad (`in`, `cm`...) o porcentaje pegados al valor. */
  private suffix(v: number): number {
    const rest = this.src.slice(this.i);
    const units = this.ctx.mode === 'length' ? LENGTH_UNITS : ANGLE_UNITS;
    const m = /^\s*(mm|cm|in|m|deg|rad|°|")(?![a-zA-Z])/.exec(rest);
    if (m && m[1] in units) {
      this.i += m[0].length;
      return v * units[m[1]];
    }
    if (/^\s*%/.test(rest)) {
      this.i += rest.indexOf('%') + 1;
      return (v / 100) * this.ctx.current;
    }
    return v;
  }
}

/** Valor de una expresión; `null` si no es válida o no es un número finito. */
export function evalExpression(text: string, ctx: { current?: number; mode?: ExprMode } = {}): number | null {
  const src = text.trim();
  if (!src) return null;
  try {
    const v = new Parser(src, { current: ctx.current ?? 0, mode: ctx.mode ?? 'length' }).parse();
    return Number.isFinite(v) ? v : null;
  } catch {
    return null;
  }
}

/**
 * Valor final de lo escrito en un campo, dado su valor actual:
 *  - `+5` suma al valor actual; `*2` y `/2` lo multiplican o dividen (`+5` = actual + 5).
 *  - `-5` es el número negativo (valor absoluto), no una resta.
 *  - `50%` es la mitad del valor actual.
 *  - Todo lo demás es un valor absoluto (`100/3`, `(10+2)*4`, `5in`).
 */
export function resolveInput(text: string, current: number, mode: ExprMode = 'length'): number | null {
  const src = text.trim();
  if (!src) return null;
  const ctx = { current, mode };
  const first = src[0];
  if (first === '+') {
    const d = evalExpression(src.slice(1), ctx);
    return d === null ? null : current + d;
  }
  if (first === '*' || first === '×') {
    const d = evalExpression(src.slice(1), ctx);
    return d === null ? null : current * d;
  }
  if (first === '/') {
    const d = evalExpression(src.slice(1), ctx);
    return d === null || d === 0 ? null : current / d;
  }
  return evalExpression(src, ctx);
}

/** Texto de un número para un campo: hasta 3 decimales, sin ceros de más. */
export function formatValue(v: number, decimals = 3): string {
  const r = Math.round(v * 10 ** decimals) / 10 ** decimals;
  return String(Object.is(r, -0) ? 0 : r);
}
