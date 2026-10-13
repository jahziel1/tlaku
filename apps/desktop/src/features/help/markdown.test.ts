import { describe, expect, it } from 'vitest';
import { classifyLink, documentTitle, parseInline, parseMarkdown, plainText, searchText, slugify } from './markdown';

describe('inline', () => {
  it('código, negrita, cursiva y escapes', () => {
    expect(parseInline('a `b*c` **d** *e* \\*f\\*')).toEqual([
      { t: 'text', v: 'a ' },
      { t: 'code', v: 'b*c' },
      { t: 'text', v: ' ' },
      { t: 'strong', c: [{ t: 'text', v: 'd' }] },
      { t: 'text', v: ' ' },
      { t: 'em', c: [{ t: 'text', v: 'e' }] },
      { t: 'text', v: ' *f*' },
    ]);
  });

  it('no confunde guiones bajos de identificadores con cursiva ni asteriscos sueltos', () => {
    expect(plainText(parseInline('snake_case_name y 2 * 3 * 4'))).toBe('snake_case_name y 2 * 3 * 4');
    expect(parseInline('_hola_')[0]).toEqual({ t: 'em', c: [{ t: 'text', v: 'hola' }] });
  });

  it('enlaces, imágenes y enlaces con paréntesis o código en el texto', () => {
    expect(parseInline('[ver `x`](a.md#b)')).toEqual([{ t: 'link', href: 'a.md#b', c: [{ t: 'text', v: 'ver ' }, { t: 'code', v: 'x' }] }]);
    expect(parseInline('![cap](../../img/a.png)')).toEqual([{ t: 'image', alt: 'cap', src: '../../img/a.png' }]);
    expect(parseInline('[u](https://x.org/a_(b))')[0]).toMatchObject({ t: 'link', href: 'https://x.org/a_(b)' });
    expect(parseInline('texto [sin cierre')).toEqual([{ t: 'text', v: 'texto [sin cierre' }]);
  });

  it('el HTML crudo queda como texto', () => {
    expect(parseInline('<script>x</script>')).toEqual([{ t: 'text', v: '<script>x</script>' }]);
  });
});

describe('bloques', () => {
  const doc = [
    '# Título *uno*',
    '',
    'Primera línea',
    'segunda línea.',
    '',
    '## Pasos y más',
    '',
    '1. Uno',
    '2. Dos con continuación',
    '   sigue aquí',
    '   - sub a',
    '   - sub b',
    '3. Tres',
    '',
    '- x',
    '- y',
    '',
    '> **Ojo:** cuidado',
    '> segunda',
    '',
    '| Col A | Col `B` |',
    '|:--|--:|',
    '| 1 | 2 |',
    '| 3 \\| 4 | 5 |',
    '',
    '```sh',
    'tlaku ports',
    '```',
    '',
    '---',
    '',
    '![Captura](../../apps/desktop/docs/screenshots/01-principal-oscuro.png)',
  ].join('\n');
  const blocks = parseMarkdown(doc);

  it('títulos con id y título del documento', () => {
    expect(documentTitle(blocks, 'x')).toBe('Título uno');
    const h2 = blocks.find((b) => b.t === 'heading' && b.level === 2);
    expect(h2).toMatchObject({ id: 'pasos-y-mas' });
    expect(documentTitle([], 'resp')).toBe('resp');
  });

  it('une las líneas de un párrafo', () => {
    const p = blocks[1];
    if (p.t !== 'p') throw new Error('se esperaba un párrafo');
    expect(plainText(p.c)).toBe('Primera línea segunda línea.');
  });

  it('lista numerada con continuación y sublista, y lista con viñetas aparte', () => {
    const lists = blocks.filter((b) => b.t === 'list');
    expect(lists).toHaveLength(2);
    const ordered = lists[0];
    if (ordered.t !== 'list') throw new Error();
    expect(ordered.ordered).toBe(true);
    expect(ordered.items).toHaveLength(3);
    expect(plainText(ordered.items[1].c)).toBe('Dos con continuación sigue aquí');
    const sub = ordered.items[1].sub[0];
    expect(sub.t === 'list' && sub.ordered === false && sub.items.length).toBe(2);
    const bullets = lists[1];
    expect(bullets.t === 'list' && bullets.ordered).toBe(false);
  });

  it('cita, tabla con alineación y barra escapada, código, línea e imagen', () => {
    const quote = blocks.find((b) => b.t === 'quote');
    expect(quote && quote.t === 'quote' && quote.c).toHaveLength(1);
    const table = blocks.find((b) => b.t === 'table');
    if (table?.t !== 'table') throw new Error();
    expect(table.align).toEqual(['left', 'right']);
    expect(plainText(table.head[1])).toBe('Col B');
    expect(plainText(table.rows[1][0])).toBe('3 | 4');
    expect(blocks.find((b) => b.t === 'code')).toMatchObject({ lang: 'sh', v: 'tlaku ports' });
    expect(blocks.some((b) => b.t === 'hr')).toBe(true);
    expect(blocks.at(-1)).toMatchObject({ t: 'image', alt: 'Captura' });
  });

  it('ids repetidos reciben sufijo y la búsqueda ignora acentos y mayúsculas', () => {
    const b = parseMarkdown('# A\n\n## Mismo\n\n## Mismo\n\nTexto con Acción');
    const ids = b.flatMap((x) => (x.t === 'heading' ? [x.id] : []));
    expect(ids).toEqual(['a', 'mismo', 'mismo-1']);
    expect(searchText(b)).toContain('accion');
  });

  it('un archivo con saltos de línea de Windows se interpreta igual', () => {
    expect(parseMarkdown('# A\r\n\r\nB\r\n')).toEqual(parseMarkdown('# A\n\nB\n'));
  });
});

describe('enlaces', () => {
  it('clasifica páginas, secciones, externos y lo demás', () => {
    expect(classifyLink('03-dibujar.md')).toEqual({ kind: 'page', file: '03-dibujar.md', anchor: '' });
    expect(classifyLink('./03-dibujar.md#nodos')).toEqual({ kind: 'page', file: '03-dibujar.md', anchor: 'nodos' });
    expect(classifyLink('#glosario')).toEqual({ kind: 'anchor', anchor: 'glosario' });
    expect(classifyLink('https://example.org/x')).toEqual({ kind: 'external', url: 'https://example.org/x' });
    expect(classifyLink('../../LICENSE')).toEqual({ kind: 'none' });
    expect(slugify('Cámara y ¿qué?')).toBe('camara-y-que');
  });
});
