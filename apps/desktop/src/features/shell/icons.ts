import {
  Circle, Crosshair, FilePlus, FileDown, FileUp, FolderOpen, Hand, Layers, MousePointer2, Moon, Pause,
  Play, Redo2, Ruler, Save, Slash, Square, SquareStop, Sun, Terminal, Type, Undo2, Waypoints, Cpu, Eye, ScanLine,
  Clipboard, ClipboardPaste, Copy, Grid3x3, History, Keyboard, Magnet, Scissors, SquareDashed,
  type LucideIcon,
} from 'lucide-react';

/** Ícono de cada comando que se muestra en barras y paletas. Los menús no llevan íconos. */
export const COMMAND_ICONS: Record<string, LucideIcon> = {
  'file.new': FilePlus,
  'file.open': FolderOpen,
  'file.save': Save,
  'file.import': FileUp,
  'file.exportGcode': FileDown,
  'edit.undo': Undo2,
  'edit.redo': Redo2,
  'edit.copy': Copy,
  'edit.cut': Scissors,
  'edit.paste': ClipboardPaste,
  'edit.pasteInPlace': Clipboard,
  'panel.history': History,
  'help.shortcuts': Keyboard,
  'tool.select': MousePointer2,
  'tool.nodes': Waypoints,
  'tool.line': Slash,
  'tool.rect': Square,
  'tool.ellipse': Circle,
  'tool.text': Type,
  'tool.pan': Hand,
  'tool.measure': Ruler,
  'view.preview': Eye,
  'view.wireframe': ScanLine,
  'view.snap': Magnet,
  'view.snapGrid': Grid3x3,
  'view.snapObjects': SquareDashed,
  'machine.frame': Crosshair,
  'machine.start': Play,
  'machine.pause': Pause,
  'machine.stop': SquareStop,
  'panel.layers': Layers,
  'panel.machine': Cpu,
  'panel.console': Terminal,
};

export const THEME_ICONS = { dark: Sun, light: Moon } as const;

