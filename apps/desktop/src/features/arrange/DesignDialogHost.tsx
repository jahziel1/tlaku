import { AlongPathDialog } from './AlongPathDialog';
import { BooleanAssistantDialog } from './BooleanAssistantDialog';
import { DockDialog } from './DockDialog';
import { HullDialog } from './HullDialog';
import { TangentDialog } from './TangentDialog';
import { VirtualArrayDialog } from './VirtualArrayDialog';
import { WarpDialog } from './WarpDialog';
import { useDesignUi } from './designUi';

export { useDesignUi } from './designUi';

/**
 * Punto de montaje de los diálogos de diseño avanzado (deformar, contorno envolvente, círculo
 * tangente, asistente booleano, copiar a lo largo de un trazo, arreglo virtual y acoplar). Lo monta
 * el shell una sola vez junto a `ArrangeDialogHost`.
 */
export function DesignDialogHost() {
  const dialog = useDesignUi((s) => s.dialog);
  switch (dialog) {
    case 'warp':
      return <WarpDialog />;
    case 'hull':
      return <HullDialog />;
    case 'tangent':
      return <TangentDialog />;
    case 'boolAssistant':
      return <BooleanAssistantDialog />;
    case 'alongPath':
      return <AlongPathDialog />;
    case 'virtualArray':
      return <VirtualArrayDialog />;
    case 'dock':
      return <DockDialog />;
    case null:
      return null;
  }
}
