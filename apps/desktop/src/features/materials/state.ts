import { create } from 'zustand';
import type { MachineClass, MaterialLibrary } from '@/api/types';
import { emptyLibrary } from './library';
import type { TestForm } from './testPlan';

/** Qué ajuste se está guardando o editando en el diálogo. */
export type EditorTarget = { kind: 'save' } | { kind: 'edit'; id: string };

interface MaterialsUiState {
  library: MaterialLibrary;
  /** Biblioteca de fábrica (solo lectura); vacía hasta que se lee. */
  factory: MaterialLibrary;
  /** Clase de la máquina activa elegida por el usuario; `null` = sin elegir. */
  machineClass: MachineClass | null;
  /** Mostrar solo los ajustes de fábrica de la clase de la máquina (y los que no dicen clase). */
  onlyClass: boolean;
  /** La biblioteca ya se leyó del disco (o se confirmó que no hay). */
  loaded: boolean;
  /** Mensaje si la lectura falló: mientras tanto no se guarda nada para no pisar el archivo. */
  loadError: string | null;
  search: string;
  /** Mostrar solo los ajustes de la máquina activa (y los que valen para cualquiera). */
  onlyDevice: boolean;
  editor: EditorTarget | null;
  testOpen: boolean;
  /** Último formulario de la prueba, para reabrirla donde se dejó durante la sesión. */
  testForm: TestForm | null;

  setLibrary: (library: MaterialLibrary) => void;
  setFactory: (factory: MaterialLibrary) => void;
  setMachineClass: (machineClass: MachineClass | null) => void;
  setOnlyClass: (onlyClass: boolean) => void;
  setLoaded: (loaded: boolean, loadError?: string | null) => void;
  setSearch: (search: string) => void;
  setOnlyDevice: (onlyDevice: boolean) => void;
  openEditor: (editor: EditorTarget) => void;
  closeEditor: () => void;
  openTest: () => void;
  closeTest: () => void;
  setTestForm: (form: TestForm | null) => void;
}

/**
 * Estado de interfaz del área de materiales (no es parte del documento). Vive aquí, como el del
 * diálogo de vectorizado, porque solo lo usan el panel, sus diálogos y el comando de la prueba.
 */
export const useMaterials = create<MaterialsUiState>()((set) => ({
  library: emptyLibrary(),
  factory: emptyLibrary(),
  machineClass: null,
  onlyClass: true,
  loaded: false,
  loadError: null,
  search: '',
  onlyDevice: false,
  editor: null,
  testOpen: false,
  testForm: null,

  setLibrary: (library) => set({ library }),
  setFactory: (factory) => set({ factory }),
  setMachineClass: (machineClass) => set({ machineClass }),
  setOnlyClass: (onlyClass) => set({ onlyClass }),
  setLoaded: (loaded, loadError = null) => set({ loaded, loadError }),
  setSearch: (search) => set({ search }),
  setOnlyDevice: (onlyDevice) => set({ onlyDevice }),
  openEditor: (editor) => set({ editor }),
  closeEditor: () => set({ editor: null }),
  openTest: () => set({ testOpen: true }),
  closeTest: () => set({ testOpen: false }),
  setTestForm: (testForm) => set({ testForm }),
}));
