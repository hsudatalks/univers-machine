import type { CSSProperties } from "react";
import type { DeveloperTarget, ManagedMachine } from "../types";
import { TerminalCard } from "./TerminalCard";
import { Button } from "./ui/button";

interface HomeMachinesPageProps {
  machines: ManagedMachine[];
  onAddMachine: () => void;
  onOpenMachine: (machineId: string) => void;
  overviewZoom: number;
  overviewZoomStyle: CSSProperties;
  pageVisible: boolean;
  resolveTarget: (targetId: string) => DeveloperTarget | undefined;
}

export function HomeMachinesPage({
  machines,
  onAddMachine,
  onOpenMachine,
  overviewZoom,
  overviewZoomStyle,
  pageVisible,
  resolveTarget,
}: HomeMachinesPageProps) {
  const machineTargets = machines
    .map((machine) => ({
      machine,
      target: resolveTarget(machine.hostTargetId),
    }))
    .filter(
      (entry): entry is { machine: ManagedMachine; target: DeveloperTarget } =>
        Boolean(entry.target),
    );

  return (
    <section className="page-section">
      {machineTargets.length > 0 ? (
        <div className="terminal-grid" style={overviewZoomStyle}>
          {machineTargets.map(({ machine, target }) => (
            <TerminalCard
              key={machine.id}
              onOpenWorkspace={() => {
                onOpenMachine(machine.id);
              }}
              pageVisible={pageVisible}
              scale={overviewZoom}
              target={target}
              title={machine.label}
            />
          ))}
        </div>
      ) : (
        <section className="server-empty-state">
          <div>
            <p className="dashboard-copy">No machines configured yet.</p>
            <Button onClick={onAddMachine} size="sm">
              Add machine
            </Button>
          </div>
        </section>
      )}
    </section>
  );
}
