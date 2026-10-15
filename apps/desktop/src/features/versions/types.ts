/** Tipos del historial de versiones (espejo de `src-tauri/src/versions/`). */

/** De dónde viene una versión. */
export type VersionKind = 'saved' | 'auto' | 'manual' | 'restore';

/** Lo que la interfaz puede pedir guardar (`saved` solo lo crea el guardado del proyecto). */
export type SnapshotKind = Exclude<VersionKind, 'saved'>;

export interface VersionInfo {
  id: string;
  kind: VersionKind;
  /** Nombre (solo las manuales). */
  label: string | null;
  /** Momento de la versión, en milisegundos desde 1970 (en las «guardado», la fecha del archivo). */
  created_ms: number;
  /** Tamaño del documento sin comprimir. */
  size_bytes: number;
  /** Lo que ocupa en disco (comprimido). */
  stored_bytes: number;
  hash: string;
  objects: number;
  layers: number;
  has_thumb: boolean;
}

export interface VersionListing {
  /** De la más nueva a la más vieja. */
  versions: VersionInfo[];
  total_bytes: number;
  /** Carpeta donde se escriben las próximas versiones. */
  dir: string;
}

/** Un proyecto con historial en la carpeta de datos de la app. */
export interface ProjectSummary {
  path: string;
  name: string;
  versions: number;
  total_bytes: number;
  latest_ms: number;
  /** `false` si el archivo ya no existe (borrado o movido). */
  exists: boolean;
}

export type VersionsLocation = 'app' | 'beside';

export interface VersionsConfig {
  enabled: boolean;
  location: VersionsLocation;
  /** Minutos entre instantáneas automáticas; 0 = sin instantáneas. */
  auto_minutes: number;
  recent_hours: number;
  daily_days: number;
  weekly_weeks: number;
  max_mb: number;
  max_versions: number;
}

/** Valores de fábrica (los mismos de `config.rs`). */
export const DEFAULT_VERSIONS_CONFIG: VersionsConfig = {
  enabled: true,
  location: 'app',
  auto_minutes: 5,
  recent_hours: 24,
  daily_days: 30,
  weekly_weeks: 26,
  max_mb: 200,
  max_versions: 500,
};
