import { AlarmBanner } from './AlarmBanner';
import { ConnectionSection } from './ConnectionSection';
import { JobSection } from './JobSection';
import { JogSection } from './JogSection';
import { PositionLostNote } from './PositionLostNote';
import { StatusSection } from './StatusSection';
import { VirtualSection } from './VirtualSection';
import './machine.css';

/** Pestaña Máquina: alarma, conexión, estado en vivo, mover el cabezal y trabajo. */
export function MachinePanel() {
  return (
    <div className="panel-scroll">
      <AlarmBanner />
      <PositionLostNote />
      <ConnectionSection />
      <VirtualSection />
      <StatusSection />
      <JogSection />
      <JobSection />
    </div>
  );
}
