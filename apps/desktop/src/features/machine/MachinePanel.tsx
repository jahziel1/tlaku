import { AlarmBanner } from './AlarmBanner';
import { ConnectionSection } from './ConnectionSection';
import { GalvoSection } from './GalvoSection';
import { GalvoStopNotice } from './GalvoNotice';
import { JobSection } from './JobSection';
import { LhySection } from './LhySection';
import { LhyStopNotice } from './LhyNotice';
import { JogSection } from './JogSection';
import { MacroButtons } from './MacroButtons';
import { MachineSettingsSection } from './MachineSettingsSection';
import { FinishSection, MoveSettingsSection } from './MoveExtras';
import { ExtraAxesSection, FireSection, PositionsSection } from './MoveWindowExtras';
import { OverridesSection } from './OverridesSection';
import { PositionLostNote } from './PositionLostNote';
import { RuidaSection } from './RuidaSection';
import { StatusSection } from './StatusSection';
import { VirtualSection } from './VirtualSection';
import './machine.css';

/**
 * Pestaña Máquina. Arriba lo que avisa (alarma, conexión en una línea si ya hay máquina, estado en
 * una franja) y lo más usado mientras se graba: Trabajo y Mover cabezal, abiertos. El resto son
 * secciones plegables cerradas por omisión (cada una recuerda cómo la dejó el usuario).
 */
export function MachinePanel() {
  return (
    <div className="panel-scroll">
      <AlarmBanner />
      <LhyStopNotice />
      <GalvoStopNotice />
      <PositionLostNote />
      <ConnectionSection />
      <StatusSection />
      <JobSection />
      <JogSection />
      <RuidaSection />
      <LhySection />
      <GalvoSection />
      <PositionsSection />
      <MacroButtons />
      <OverridesSection />
      <ExtraAxesSection />
      <FinishSection />
      <FireSection />
      <MoveSettingsSection />
      <MachineSettingsSection />
      <VirtualSection />
    </div>
  );
}
