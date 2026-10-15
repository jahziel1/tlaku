import type { ProbeResult } from '@/api/api';
import { numericSettings } from './compare';
import { CATALOG, type CatalogEntry, type Signature } from './catalog';
import { detectionFrom, isCompatible } from './filter';

/** Un modelo del catálogo cuya firma coincidió con lo que la máquina contestó. */
export interface SignatureMatch {
  entry: CatalogEntry;
  signature: Signature;
  /** Líneas de la máquina que coincidieron (las exigidas primero, luego las `show`), sin repetir. */
  evidence: string[];
  /** La cama leída de `$130/$131` es la de la firma (solo informa si la firma la trae). */
  bedMatches: boolean;
}

export interface ModelGuess {
  /** El modelo que se preselecciona: la única firma que coincidió o la que gana por la cama. */
  best: SignatureMatch | null;
  /** Todas las que coincidieron, la mejor primero. Con más de una el usuario ve las candidatas. */
  candidates: SignatureMatch[];
}

export const NO_GUESS: ModelGuess = { best: null, candidates: [] };

/** Líneas del saludo y de `$I`, tal como las dijo la placa. */
export function probeLines(result: ProbeResult): string[] {
  return [...result.banner.split('\n'), ...result.info].map((l) => l.trim()).filter(Boolean);
}

/**
 * Potencia del láser que dice la placa en `$I` o en el saludo (la Falcon2 escribe
 * `inquire_info->power:22W ,CV50-LASER-22W-Release…`), o `null`. Solo se lee la línea `power:`
 * para no confundir con otros números; es un dato de la máquina real, mejor que el del catálogo.
 */
export function wattsFromProbe(result: ProbeResult | null): number | null {
  if (!result) return null;
  for (const line of probeLines(result)) {
    const m = /\bpower\s*:\s*(\d+(?:\.\d+)?)\s*w\b/i.exec(line);
    if (m) {
      const w = Number(m[1]);
      if (Number.isFinite(w) && w > 0 && w <= 1000) return w;
    }
  }
  return null;
}

const SAME_EPS = 0.5;

/**
 * Busca en el catálogo los modelos cuya firma aparece en el saludo o en `$I`. Una firma coincide
 * solo si TODAS sus cadenas aparecen (sin importar mayúsculas). Solo se consideran modelos del mismo
 * tipo de firmware que se detectó. Si varias firmas coinciden, la cama (`$130/$131`) desempata: gana
 * la que la tiene igual a la leída, siempre que sea única; si no, no hay preselección y se muestran
 * todas como candidatas.
 */
export function identifyModel(result: ProbeResult | null, entries: readonly CatalogEntry[] = CATALOG): ModelGuess {
  if (!result || result.family === 'unknown') return NO_GUESS;
  const detection = detectionFrom(result);
  const lines = probeLines(result);
  const lower = lines.map((l) => l.toLowerCase());
  const settings = numericSettings(result.settings);
  const bed: [number, number] | null = settings[130] !== undefined && settings[131] !== undefined ? [settings[130], settings[131]] : null;

  const firstLine = (needle: string) => lower.findIndex((l) => l.includes(needle));
  const found: SignatureMatch[] = [];
  for (const entry of entries) {
    if (!entry.signatures || !isCompatible(entry, detection)) continue;
    for (const signature of entry.signatures) {
      if (signature.contains.length === 0) continue;
      const idx = signature.contains.map(firstLine);
      if (idx.some((i) => i < 0)) continue;
      const extra = (signature.show ?? []).map(firstLine).filter((i) => i >= 0);
      const evidence = [...new Set([...idx, ...extra])].map((i) => lines[i]!);
      const bedMatches = !!signature.bed && !!bed && Math.abs(signature.bed[0] - bed[0]) < SAME_EPS && Math.abs(signature.bed[1] - bed[1]) < SAME_EPS;
      found.push({ entry, signature, evidence, bedMatches });
      break; // una firma por modelo basta
    }
  }
  // Primero las que coinciden en cama; entre ellas, las que exigen más cadenas (más específicas).
  found.sort((a, b) => Number(b.bedMatches) - Number(a.bedMatches) || b.signature.contains.length - a.signature.contains.length);
  if (found.length === 0) return NO_GUESS;
  if (found.length === 1) return { best: found[0]!, candidates: found };
  const [first, second] = found as [SignatureMatch, SignatureMatch, ...SignatureMatch[]];
  const wins = first.bedMatches !== second.bedMatches || first.signature.contains.length !== second.signature.contains.length;
  return { best: wins ? first : null, candidates: found };
}
