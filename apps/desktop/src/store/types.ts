import type { StateCreator } from 'zustand';
import type { DevicesSlice } from './devices';
import type { DocumentSlice } from './document';
import type { MachineSlice } from './machine';
import type { NoticesSlice } from './notices';
import type { PreviewSlice } from './preview';
import type { SelectionSlice } from './selection';
import type { ViewSlice } from './view';

/** Estado completo de la aplicación: unión de todos los slices. */
export type AppState = DocumentSlice &
  SelectionSlice &
  ViewSlice &
  MachineSlice &
  PreviewSlice &
  DevicesSlice &
  NoticesSlice;

/** Forma de cada `createXSlice`. Un slice puede leer a los demás con `get()`. */
export type SliceCreator<T> = StateCreator<AppState, [], [], T>;
