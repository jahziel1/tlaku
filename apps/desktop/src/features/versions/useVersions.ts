import { useCallback, useEffect, useState } from 'react';
import { getApi } from '@/api';
import { errorMessage } from '@/lib/errors';
import type { ProjectSummary, VersionListing } from './types';

export interface VersionsData {
  listing: VersionListing | null;
  projects: ProjectSummary[];
  loading: boolean;
  error: string | null;
  reload: () => Promise<void>;
}

/** Miniaturas ya pedidas (por proyecto y versión): una versión no cambia, así que no se piden dos veces. */
const thumbs = new Map<string, string | null>();

export function thumbKey(path: string, id: string): string {
  return `${path}\u0000${id}`;
}

/** La miniatura (texto SVG) de una versión, o `null` si no tiene. */
export async function loadThumb(path: string, id: string): Promise<string | null> {
  const key = thumbKey(path, id);
  if (thumbs.has(key)) return thumbs.get(key) ?? null;
  const svg = await getApi().versionsThumb({ path, id });
  thumbs.set(key, svg);
  return svg;
}

/** Olvida las miniaturas guardadas de un proyecto (al borrar versiones). */
export function forgetThumbs(path: string): void {
  for (const key of [...thumbs.keys()]) if (key.startsWith(`${path}\u0000`)) thumbs.delete(key);
}

/** Versiones del proyecto `path` (si lo hay) y la lista de otros proyectos con historial. */
export function useVersions(path: string | null): VersionsData {
  const [listing, setListing] = useState<VersionListing | null>(null);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      const api = getApi();
      const [list, projs] = await Promise.all([path ? api.versionsList({ path }) : Promise.resolve(null), api.versionsProjects()]);
      setListing(list);
      setProjects(projs);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, [path]);

  useEffect(() => {
    setListing(null);
    void reload();
  }, [reload]);

  return { listing, projects, loading, error, reload };
}
