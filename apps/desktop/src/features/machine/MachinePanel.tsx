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
import { OverridesSection } from './OverridesSection';
import { PositionLostNote } from './PositionLostNote';
import { RuidaSection } from './RuidaSection';
import { StatusSection } from './StatusSection';
import { VirtualSection } from './VirtualSection';
import './machine.css';

/** Pestaña Máquina: alarma, conexión, estado en vivo, mover el cabezal, trabajo y ajustes en vivo. */
export function MachinePanel() {
  return (
    <div className="panel-scroll">
      <AlarmBanner />
      <LhyStopNotice />
      <GalvoStopNotice />
      <PositionLostNote />
      <ConnectionSection />
      <VirtualSection />
      <RuidaSection />
      <LhySection />
      <GalvoSection />
      <StatusSection />
      <JogSection />
      <MacroButtons />
      <JobSection />
      <OverridesSection />
      <MachineSettingsSection />
    </div>
  );
}
