/** Tablero de calibración para imprimir: SVG y PDF mínimo, ambos en milímetros reales. */

export type PageKind = 'a4' | 'letter';

/** Hoja apaisada (ancho, alto) en mm. */
export const PAGES: Record<PageKind, [number, number]> = {
  a4: [297, 210],
  letter: [279.4, 215.9],
};

export const BOARD_CORNERS: [number, number] = [9, 6];
/** Margen mínimo (mm) que se deja libre por la zona no imprimible y para la leyenda. */
const MARGIN = 10;
const CAPTION_H = 12;

export interface BoardLayout {
  page: [number, number];
  /** Cuadros del tablero: esquinas internas + 1 por lado. */
  squares: [number, number];
  squareMm: number;
  /** Esquina superior izquierda del tablero en la hoja (mm). */
  origin: [number, number];
  /** Cuadros oscuros: x, y, lado (mm, Y hacia abajo desde arriba de la hoja). */
  dark: [number, number, number][];
  fits: boolean;
  /** Cuadro más grande que cabe en la hoja. */
  maxSquareMm: number;
}

export function maxSquare(page: PageKind, corners: readonly [number, number] = BOARD_CORNERS): number {
  const [pw, ph] = PAGES[page];
  const [c, r] = corners;
  return Math.floor(Math.min((pw - 2 * MARGIN) / (c + 1), (ph - 2 * MARGIN - CAPTION_H) / (r + 1)) * 10) / 10;
}

export function boardLayout(page: PageKind, squareMm: number, corners: readonly [number, number] = BOARD_CORNERS): BoardLayout {
  const [pw, ph] = PAGES[page];
  const squares: [number, number] = [corners[0] + 1, corners[1] + 1];
  const w = squares[0] * squareMm;
  const h = squares[1] * squareMm;
  const origin: [number, number] = [(pw - w) / 2, (ph - CAPTION_H - h) / 2];
  const dark: [number, number, number][] = [];
  for (let j = 0; j < squares[1]; j++)
    for (let i = 0; i < squares[0]; i++) if ((i + j) % 2 === 0) dark.push([origin[0] + i * squareMm, origin[1] + j * squareMm, squareMm]);
  const max = maxSquare(page, corners);
  return { page: [pw, ph], squares, squareMm, origin, dark, fits: squareMm > 0 && squareMm <= max, maxSquareMm: max };
}

const f = (n: number) => String(Math.round(n * 1000) / 1000);

function escapeXml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

/** SVG de la hoja completa; `width`/`height` en mm para que se imprima a escala real. */
export function boardSvg(layout: BoardLayout, caption: string): string {
  const [pw, ph] = layout.page;
  const rects = layout.dark.map(([x, y, s]) => `<rect x="${f(x)}" y="${f(y)}" width="${f(s)}" height="${f(s)}"/>`).join('');
  return (
    `<?xml version="1.0" encoding="UTF-8"?>\n` +
    `<svg xmlns="http://www.w3.org/2000/svg" width="${f(pw)}mm" height="${f(ph)}mm" viewBox="0 0 ${f(pw)} ${f(ph)}">` +
    `<rect width="${f(pw)}" height="${f(ph)}" fill="#fff"/>` +
    `<g fill="#000" shape-rendering="crispEdges">${rects}</g>` +
    `<text x="${f(pw / 2)}" y="${f(ph - MARGIN / 2 - 1)}" font-family="Helvetica, Arial, sans-serif" font-size="3.5" text-anchor="middle" fill="#000">${escapeXml(caption)}</text>` +
    `</svg>\n`
  );
}

const PT = 72 / 25.4;

/** Texto para un literal PDF en WinAnsi: acentos como escapes octales, lo demás ASCII. */
export function pdfText(s: string): string {
  let out = '';
  for (const ch of s) {
    const code = ch.codePointAt(0)!;
    if (ch === '(' || ch === ')' || ch === '\\') out += `\\${ch}`;
    else if (code >= 32 && code < 127) out += ch;
    else if (code >= 160 && code <= 255) out += `\\${code.toString(8).padStart(3, '0')}`;
    else out += '-';
  }
  return out;
}

/** PDF de una página con el tablero (vectorial, sin imágenes) y la leyenda en Helvetica. */
export function boardPdf(layout: BoardLayout, caption: string): Uint8Array {
  const [pw, ph] = layout.page;
  const content: string[] = ['0 g'];
  for (const [x, y, s] of layout.dark) content.push(`${f(x * PT)} ${f((ph - y - s) * PT)} ${f(s * PT)} ${f(s * PT)} re f`);
  const capY = (MARGIN / 2 + 1) * PT;
  const size = 10;
  // Centrado aproximado: Helvetica mide ~0.5 em de media por carácter.
  const textW = caption.length * size * 0.5;
  content.push(`BT /F1 ${size} Tf ${f((pw * PT - textW) / 2)} ${f(capY)} Td (${pdfText(caption)}) Tj ET`);
  const stream = content.join('\n');
  const objs = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${f(pw * PT)} ${f(ph * PT)}] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>`,
    `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`,
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>',
  ];
  let out = '%PDF-1.4\n';
  const offsets: number[] = [];
  objs.forEach((o, i) => {
    offsets.push(out.length);
    out += `${i + 1} 0 obj\n${o}\nendobj\n`;
  });
  const xref = out.length;
  out += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n`;
  for (const off of offsets) out += `${String(off).padStart(10, '0')} 00000 n \n`;
  out += `trailer\n<< /Size ${objs.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  // Todo es ASCII (los acentos van escapados), así que cada carácter es un byte.
  return Uint8Array.from(out, (c) => c.charCodeAt(0));
}

export function bytesToBase64(bytes: Uint8Array): string {
  let bin = '';
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}

export function textToBase64(text: string): string {
  return bytesToBase64(new TextEncoder().encode(text));
}
