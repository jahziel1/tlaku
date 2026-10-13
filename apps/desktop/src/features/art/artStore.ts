import { create } from 'zustand';
import type { ArtCategory } from './catalog';
import type { UserArt } from './types';

interface ArtState {
  open: boolean;
  category: ArtCategory | 'usuario' | 'todo';
  query: string;
  user: UserArt[];
  /** Mensaje de error de la última operación con la biblioteca del usuario. */
  error: string | null;
  setOpen: (open: boolean) => void;
  setCategory: (c: ArtState['category']) => void;
  setQuery: (q: string) => void;
  setUser: (user: UserArt[]) => void;
  setError: (error: string | null) => void;
}

/** Estado de interfaz del panel de arte (no es parte del documento). */
export const useArtPanel = create<ArtState>()((set) => ({
  open: false,
  category: 'todo',
  query: '',
  user: [],
  error: null,
  setOpen: (open) => set({ open }),
  setCategory: (category) => set({ category }),
  setQuery: (query) => set({ query }),
  setUser: (user) => set({ user }),
  setError: (error) => set({ error }),
}));
