import { useMemo } from "react";
import type { AppBootstrap, DeveloperTarget, ManagedMachine } from "../types";
import type { ActiveView } from "../lib/view-types";
import { isMachineHostTarget, visibleContainers } from "../lib/container-visibility";

type OverviewContainerEntry = {
  container: ManagedMachine["containers"][number];
  machine: ManagedMachine;
  target: DeveloperTarget | undefined;
};

function machineForTargetId(
  machines: ManagedMachine[],
  targetId: string,
): ManagedMachine | undefined {
  return machines.find(
    (machine) =>
      machine.hostTargetId === targetId ||
      machine.containers.some((container) => container.targetId === targetId),
  );
}

export function useWorkbenchInventory(
  bootstrap: AppBootstrap | null,
  activeView: ActiveView,
  visitedMachineIds: string[],
) {
  const targetById = useMemo(
    () =>
      new Map(
        (bootstrap?.targets ?? []).map((target) => [
          target.id,
          target,
        ]),
      ),
    [bootstrap],
  );

  const managedTargetIds = useMemo(
    () =>
      new Set(
        bootstrap?.machines.flatMap((machine) =>
          machine.containers.map((container) => container.targetId),
        ) ?? [],
      ),
    [bootstrap],
  );

  const standaloneTargets = useMemo(
    () =>
      bootstrap?.targets.filter(
        (target) =>
          !managedTargetIds.has(target.id) && !isMachineHostTarget(target),
      ) ?? [],
    [bootstrap, managedTargetIds],
  );

  const overviewContainers = useMemo<OverviewContainerEntry[]>(
    () =>
      bootstrap?.machines.flatMap((machine) =>
        visibleContainers(machine.containers).map((container) => ({
          container,
          machine,
          target: targetById.get(container.targetId),
        })),
      ) ?? [],
    [bootstrap, targetById],
  );

  const overviewTerminalTargets = useMemo(
    () => [
      ...overviewContainers
        .map((entry) => entry.target)
        .filter((target): target is DeveloperTarget => Boolean(target)),
      ...standaloneTargets,
    ],
    [overviewContainers, standaloneTargets],
  );

  const visitedMachines = useMemo(
    () =>
      visitedMachineIds
        .map((machineId) => bootstrap?.machines.find((machine) => machine.id === machineId))
        .filter((machine): machine is ManagedMachine => Boolean(machine)),
    [bootstrap, visitedMachineIds],
  );

  const activeContainerTarget = useMemo(() => {
    if (activeView.kind !== "container") {
      return undefined;
    }

    return targetById.get(activeView.targetId);
  }, [activeView, targetById]);

  const activeContainerMachine = useMemo(
    () =>
      activeContainerTarget
        ? machineForTargetId(bootstrap?.machines ?? [], activeContainerTarget.id)
        : undefined,
    [activeContainerTarget, bootstrap],
  );

  const reachableContainerCount = useMemo(
    () =>
      overviewContainers.filter((entry) => entry.container.sshReachable).length,
    [overviewContainers],
  );

  return {
    activeContainerMachine,
    activeContainerTarget,
    overviewContainers,
    overviewTerminalTargets,
    reachableContainerCount,
    standaloneTargets,
    targetById,
    visitedMachines,
  };
}
