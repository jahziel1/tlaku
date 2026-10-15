import { create } from 'zustand';
import type { CatalogState } from '@/api/types';
import { EMPTY_FILTER, type CommunityFilter } from './communityModel';

/** `idle` = sin pedir nada todavía; `loading` = descargando; `done` = terminó (bien o con error en `catalog.error`). */
export type CommunityStatus = 'idle' | 'loading' | 'done';

interface CommunityUiState {
  open: boolean;
  /** Lo último que dijo el motor (índice validado, su fecha y de dónde salió); `null` antes de la primera lectura. */
  catalog: CatalogState | null;
  status: CommunityStatus;
  filter: CommunityFilter;
  /** `id` del ajuste cuyo detalle se ve. */
  selectedId: string | null;

  setOpen: (open: boolean) => void;
  setCatalog: (catalog: CatalogState | null) => void;
  setStatus: (status: CommunityStatus) => void;
  setFilter: (patch: Partial<CommunityFilter>) => void;
  select: (id: string | null) => void;
}

/**
 * Estado de interfaz del navegador de la comunidad (no es parte del documento ni se guarda). El filtro
 * vuelve a su valor inicial cada vez que se abre: por omisión, mi máquina.
 */
export const useCommunity = create<CommunityUiState>()((set) => ({
  open: false,
  catalog: null,
  status: 'idle',
  filter: EMPTY_FILTER,
  selectedId: null,

  setOpen: (open) => set(open ? { open, filter: EMPTY_FILTER, selectedId: null } : { open }),
  setCatalog: (catalog) => set({ catalog }),
  setStatus: (status) => set({ status }),
  setFilter: (patch) => set((s) => ({ filter: { ...s.filter, ...patch } })),
  select: (selectedId) => set({ selectedId }),
}));
