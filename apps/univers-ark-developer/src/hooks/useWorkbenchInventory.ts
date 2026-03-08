import { useMemo } from "react";
import type { AppBootstrap, DeveloperTarget, ManagedServer } from "../types";
import type { ActiveView } from "../lib/view-types";

type OverviewContainerEntry = {
  container: ManagedServer["containers"][number];
  server: ManagedServer;
  target: DeveloperTarget | undefined;
};

function serverForTargetId(
  servers: ManagedServer[],
  targetId: string,
): ManagedServer | undefined {
  return servers.find((server) =>
    server.containers.some((container) => container.targetId === targetId),
  );
}

export function useWorkbenchInventory(
  bootstrap: AppBootstrap | null,
  activeView: ActiveView,
  visitedServerIds: string[],
) {
  const targetById = useMemo(
    () => new Map(bootstrap?.targets.map((target) => [target.id, target]) ?? []),
    [bootstrap],
  );

  const managedTargetIds = useMemo(
    () =>
      new Set(
        bootstrap?.servers.flatMap((server) =>
          server.containers.map((container) => container.targetId),
        ) ?? [],
      ),
    [bootstrap],
  );

  const standaloneTargets = useMemo(
    () =>
      bootstrap?.targets.filter((target) => !managedTargetIds.has(target.id)) ?? [],
    [bootstrap, managedTargetIds],
  );

  const overviewContainers = useMemo<OverviewContainerEntry[]>(
    () =>
      bootstrap?.servers.flatMap((server) =>
        server.containers.map((container) => ({
          container,
          server,
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

  const visitedServers = useMemo(
    () =>
      visitedServerIds
        .map((serverId) => bootstrap?.servers.find((server) => server.id === serverId))
        .filter((server): server is ManagedServer => Boolean(server)),
    [bootstrap, visitedServerIds],
  );

  const activeContainerTarget = useMemo(() => {
    if (!bootstrap || activeView.kind !== "container") {
      return undefined;
    }

    return bootstrap.targets.find((target) => target.id === activeView.targetId);
  }, [activeView, bootstrap]);

  const activeContainerServer = useMemo(
    () =>
      activeContainerTarget
        ? serverForTargetId(bootstrap?.servers ?? [], activeContainerTarget.id)
        : undefined,
    [activeContainerTarget, bootstrap],
  );

  const reachableContainerCount = useMemo(
    () =>
      overviewContainers.filter((entry) => entry.container.sshReachable).length,
    [overviewContainers],
  );

  return {
    activeContainerServer,
    activeContainerTarget,
    overviewContainers,
    overviewTerminalTargets,
    reachableContainerCount,
    standaloneTargets,
    targetById,
    visitedServers,
  };
}
