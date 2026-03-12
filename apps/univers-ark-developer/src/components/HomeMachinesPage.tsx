import type { CSSProperties } from "react";
import type { DeveloperTarget, ManagedMachine } from "../types";
import { TerminalCard } from "./TerminalCard";
import { Button } from "./ui/button";

interface HomeMachinesPageProps {
  activeFocusedTargetId: string;
  machines: ManagedMachine[];
  onAddMachine: () => void;
  onFocusTarget: (targetId: string) => void;
  onOpenMachine: (machineId: string) => void;
  overviewZoom: number;
  overviewZoomStyle: CSSProperties;
  pageVisible: boolean;
  registerOverviewCardElement: (targetId: string, element: HTMLElement | null) => void;
  resolveTarget: (targetId: string) => DeveloperTarget | undefined;
}

export function HomeMachinesPage({
  activeFocusedTargetId,
  machines,
  onAddMachine,
  onFocusTarget,
  onOpenMachine,
  overviewZoom,
  overviewZoomStyle,
  pageVisible,
  registerOverviewCardElement,
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
              isGridFocused={target.id === activeFocusedTargetId}
              key={machine.id}
              onFocusRequest={() => {
                onFocusTarget(target.id);
              }}
              onOpenWorkspace={() => {
                onOpenMachine(machine.id);
              }}
              pageVisible={pageVisible}
              registerElement={(element) => {
                registerOverviewCardElement(target.id, element);
              }}
              scale={overviewZoom}
              target={target}
              title={`${machine.label} (Machine provider)`}
            />
          ))}
        </div>
      ) : (
        <section className="server-empty-state">
          <div>
            <p className="dashboard-copy">No providers configured yet.</p>
            <Button onClick={onAddMachine} size="sm">
              Add provider
            </Button>
          </div>
        </section>
      )}
    </section>
  );
}
