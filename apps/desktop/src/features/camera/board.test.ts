import { describe, expect, it } from 'vitest';
import { BOARD_CORNERS, PAGES, boardLayout, boardPdf, boardSvg, bytesToBase64, maxSquare, pdfText } from './board';

describe('tablero imprimible', () => {
  it('9x6 esquinas internas son 10x7 cuadros, 35 oscuros', () => {
    const l = boardLayout('a4', 25);
    expect(l.squares).toEqual([BOARD_CORNERS[0] + 1, BOARD_CORNERS[1] + 1]);
    expect(l.dark).toHaveLength(35);
    expect(l.fits).toBe(true);
  });

  it('queda centrado en la hoja y con cuadros contiguos del tamaño pedido', () => {
    const l = boardLayout('a4', 25);
    expect(l.origin[0]).toBeCloseTo((297 - 250) / 2, 6);
    const xs = new Set(l.dark.map((d) => d[0]));
    expect(xs.size).toBe(10);
    expect(l.dark[0]).toEqual([l.origin[0], l.origin[1], 25]);
  });

  it('avisa cuando el cuadro no cabe y da el máximo', () => {
    const l = boardLayout('a4', 40);
    expect(l.fits).toBe(false);
    expect(l.maxSquareMm).toBe(maxSquare('a4'));
    expect(l.maxSquareMm).toBeGreaterThanOrEqual(25);
    expect(boardLayout('letter', maxSquare('letter')).fits).toBe(true);
  });

  it('el SVG mide la hoja en mm reales y trae 35 cuadros', () => {
    const svg = boardSvg(boardLayout('a4', 25), 'Tablero & prueba <1>');
    expect(svg).toContain('width="297mm" height="210mm"');
    expect(svg.match(/<rect x=/g)).toHaveLength(35);
    expect(svg).toContain('Tablero &amp; prueba &lt;1&gt;');
  });

  it('el PDF es válido: tabla xref con los desplazamientos correctos', () => {
    const bytes = boardPdf(boardLayout('letter', 20), 'Tlaku: cuadro de 20 mm (100 %) ñ');
    const text = new TextDecoder('latin1').decode(bytes);
    expect(text.startsWith('%PDF-1.4')).toBe(true);
    expect(text.trimEnd().endsWith('%%EOF')).toBe(true);
    const xref = Number(/startxref\n(\d+)/.exec(text)![1]);
    expect(text.slice(xref, xref + 4)).toBe('xref');
    const entries = [...text.slice(xref).matchAll(/(\d{10}) 00000 n /g)].map((m) => Number(m[1]));
    expect(entries).toHaveLength(5);
    entries.forEach((off, i) => expect(text.slice(off, off + `${i + 1} 0 obj`.length)).toBe(`${i + 1} 0 obj`));
    expect(text.match(/ re f/g)).toHaveLength(35);
    // MediaBox de carta apaisada en puntos.
    expect(text).toContain(`/MediaBox [0 0 ${Math.round(PAGES.letter[0] * (72 / 25.4) * 1000) / 1000}`);
    // La longitud declarada del flujo coincide con el flujo.
    const len = Number(/\/Length (\d+)/.exec(text)![1]);
    const body = /stream\n([\s\S]*?)\nendstream/.exec(text)![1]!;
    expect(body.length).toBe(len);
  });

  it('escapa el texto del PDF (paréntesis, barra y acentos en WinAnsi)', () => {
    expect(pdfText('a(b)\\c')).toBe('a\\(b\\)\\\\c');
    expect(pdfText('ñ á')).toBe('\\361 \\341');
    expect(pdfText('€')).toBe('-');
  });

  it('codifica a base64', () => {
    expect(bytesToBase64(new TextEncoder().encode('Tlaku'))).toBe('VGxha3U=');
    expect(bytesToBase64(new Uint8Array(100000)).length).toBe(Math.ceil(100000 / 3) * 4);
  });
});
