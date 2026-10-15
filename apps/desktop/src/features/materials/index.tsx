import { useEffect } from 'react';
import { loadMaterials } from './actions';
import { CommunityBrowser } from './CommunityBrowser';
import { useCommunity } from './communityStore';
import { PresetDialogHost } from './PresetDialog';
import { useMaterials } from './state';
import { TestDialog } from './TestDialog';

export { MaterialsPanel } from './MaterialsPanel';

/**
 * Punto de montaje del área `materials` (lo monta el shell una sola vez): carga la biblioteca y
 * aloja el diálogo de prueba de materiales, el de guardar o editar un ajuste y el navegador de la comunidad.
 */
export function MaterialsDialogHost() {
  const testOpen = useMaterials((s) => s.testOpen);
  const communityOpen = useCommunity((s) => s.open);

  useEffect(() => {
    void loadMaterials();
  }, []);

  return (
    <>
      {testOpen && <TestDialog />}
      {communityOpen && <CommunityBrowser />}
      <PresetDialogHost />
    </>
  );
}
