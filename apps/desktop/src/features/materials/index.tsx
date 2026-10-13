import { useEffect } from 'react';
import { loadMaterials } from './actions';
import { PresetDialogHost } from './PresetDialog';
import { useMaterials } from './state';
import { TestDialog } from './TestDialog';

export { MaterialsPanel } from './MaterialsPanel';

/**
 * Punto de montaje del área `materials` (lo monta el shell una sola vez): carga la biblioteca y
 * aloja el diálogo de prueba de materiales y el de guardar o editar un ajuste.
 */
export function MaterialsDialogHost() {
  const testOpen = useMaterials((s) => s.testOpen);

  useEffect(() => {
    void loadMaterials();
  }, []);

  return (
    <>
      {testOpen && <TestDialog />}
      <PresetDialogHost />
    </>
  );
}
