import { create } from 'zustand';
import type { Placement } from './components';
import type { ArtCategory } from './catalog';
import type { UserArt, UserComponent } from './types';

interface ArtState {
  open: boolean;
  category: ArtCategory | 'usuario' | 'componentes' | 'todo';
  query: string;
  user: UserArt[];
  /** Componentes guardados (issue #22). */
  components: UserComponent[];
  /** Dónde cae un componente al insertarlo con un clic. */
  placement: Placement;
  /** Diálogo «Guardar como componente»: los ids que se guardarán (`null` = cerrado). */
  saving: string[] | null;
  /** Mensaje de error de la última operación con la biblioteca del usuario. */
  error: string | null;
  setOpen: (open: boolean) => void;
  setCategory: (c: ArtState['category']) => void;
  setQuery: (q: string) => void;
  setUser: (user: UserArt[]) => void;
  setComponents: (components: UserComponent[]) => void;
  setPlacement: (placement: Placement) => void;
  setSaving: (ids: string[] | null) => void;
  setError: (error: string | null) => void;
}

/** Estado de interfaz del panel de arte (no es parte del documento). */
export const useArtPanel = create<ArtState>()((set) => ({
  open: false,
  category: 'todo',
  query: '',
  user: [],
  components: [],
  placement: 'auto',
  saving: null,
  error: null,
  setOpen: (open) => set({ open }),
  setCategory: (category) => set({ category }),
  setQuery: (query) => set({ query }),
  setUser: (user) => set({ user }),
  setComponents: (components) => set({ components }),
  setPlacement: (placement) => set({ placement }),
  setSaving: (saving) => set({ saving }),
  setError: (error) => set({ error }),
}));
