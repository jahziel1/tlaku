import type { CodeKind, CodeSpec, DesignObject, Document, ImageObject, QrEcc, VariableCode, VariableImage, VectorObject } from '@/api/types';
import { isTextObject } from '@/features/text/signature';
import { hasVariables, shiftTemplate } from './syntax';

/**
 * Objetos del diseño cuyo contenido puede llevar variables, además de los textos: una imagen
 * cuyo archivo sale de una columna (`ImageObject.variable`) y un QR o código de barras cuyo
 * contenido es una plantilla (`VectorObject.code`).
 */

export type VariableImageObject = DesignObject & { kind: { type: 'image' } & ImageObject & { variable: VariableImage } };
export type VariableCodeObject = DesignObject & { kind: { type: 'vector' } & VectorObject & { code: VariableCode } };

export const isVariableImage = (o: DesignObject): o is VariableImageObject => o.kind.type === 'image' && !!o.kind.variable;
export const isVariableCode = (o: DesignObject): o is VariableCodeObject => o.kind.type === 'vector' && !!o.kind.code;

/** Plantillas de un objeto: el texto, la ruta de la imagen variable o el contenido del código. */
export function objectTemplates(o: DesignObject): string[] {
  if (isTextObject(o)) return [o.kind.text];
  if (isVariableImage(o)) return [o.kind.variable.source];
  if (isVariableCode(o)) return [o.kind.code.data];
  return [];
}

/** ¿El objeto lleva alguna variable (no solo llaves escapadas)? */
export function hasObjectVariables(o: DesignObject): boolean {
  return objectTemplates(o).some(hasVariables);
}

/** El mismo objeto con sus plantillas desplazadas `k` pasos (para "una copia por fila"). */
export function shiftObject(o: DesignObject, k: number): DesignObject {
  if (k === 0) return o;
  if (isTextObject(o)) return { ...o, kind: { ...o.kind, text: shiftTemplate(o.kind.text, k) } };
  if (isVariableImage(o)) return { ...o, kind: { ...o.kind, variable: { ...o.kind.variable, source: shiftTemplate(o.kind.variable.source, k) } } };
  if (isVariableCode(o)) return { ...o, kind: { ...o.kind, code: { ...o.kind.code, data: shiftTemplate(o.kind.code.data, k) } } };
  return o;
}

/** Imágenes variables del documento. */
export function variableImages(doc: Pick<Document, 'objects'>): VariableImageObject[] {
  return doc.objects.filter(isVariableImage);
}

/** Códigos variables (con al menos una variable en el contenido) del documento. */
export function variableCodes(doc: Pick<Document, 'objects'>): VariableCodeObject[] {
  return doc.objects.filter((o): o is VariableCodeObject => isVariableCode(o) && hasVariables(o.kind.code.data));
}

/** Ajustes del generador a partir de los de un código variable, con el contenido ya resuelto. */
export function codeSpecOf(code: VariableCode, data: string): CodeSpec {
  return {
    kind: code.kind as CodeKind,
    data,
    ecc: code.ecc as QrEcc,
    size_mm: code.size_mm,
    height_mm: code.height_mm,
    margin_mm: code.margin_mm,
    human_readable: code.human_readable,
    invert: code.invert,
    font_family: code.font_family,
  };
}

/** Lo contrario: los ajustes del diálogo de códigos guardados en el objeto. */
export function variableCodeOf(spec: CodeSpec): VariableCode {
  return { ...spec };
}
