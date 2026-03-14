import { ServerPage } from "../../components/ServerPage";
import type { DeveloperTarget, ManagedMachine } from "../../types";

interface ProviderPagesProps {
  activeMachineId: string | null;
  onOpenProviderSettings: (machineId: string) => void;
  onOpenWorkbench: (targetId: string) => void;
  pageVisible: boolean;
  resolveTarget: (targetId: string) => DeveloperTarget | undefined;
  visitedMachines: ManagedMachine[];
}

export function ProviderPages({
  activeMachineId,
  onOpenProviderSettings,
  onOpenWorkbench,
  pageVisible,
  resolveTarget,
  visitedMachines,
}: ProviderPagesProps) {
  return (
    <>
      {visitedMachines.map((machine) => (
        <section
          key={machine.id}
          className={`content-page ${
            pageVisible && activeMachineId === machine.id ? "" : "is-hidden"
          }`}
        >
          <ServerPage
            onOpenSettings={() => {
              onOpenProviderSettings(machine.id);
            }}
            onOpenWorkspace={onOpenWorkbench}
            pageVisible={pageVisible && activeMachineId === machine.id}
            resolveTarget={resolveTarget}
            server={machine}
          />
        </section>
      ))}
    </>
  );
}
