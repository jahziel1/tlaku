import type { StateCreator } from 'zustand';
import type { CameraSlice } from './camera';
import type { DevicesSlice } from './devices';
import type { DocumentSlice } from './document';
import type { GalvoSlice } from './galvo';
import type { MachineSlice } from './machine';
import type { MoveControlSlice } from './moveControl';
import type { NoticesSlice } from './notices';
import type { PreviewSlice } from './preview';
import type { PrefsSlice } from './prefs';
import type { SelectionSlice } from './selection';
import type { TabsSlice } from './tabs';
import type { ViewSlice } from './view';

/** Estado completo de la aplicación: unión de todos los slices. */
export type AppState = DocumentSlice &
  SelectionSlice &
  ViewSlice &
  MachineSlice &
  GalvoSlice &
  MoveControlSlice &
  PreviewSlice &
  DevicesSlice &
  NoticesSlice &
  CameraSlice &
  PrefsSlice &
  TabsSlice;

/** Forma de cada `createXSlice`. Un slice puede leer a los demás con `get()`. */
export type SliceCreator<T> = StateCreator<AppState, [], [], T>;
