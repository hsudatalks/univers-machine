import { useMemo } from "react";
import type { AppBootstrap, DeveloperTarget, ManagedServer } from "../types";
import type { ActiveView } from "../lib/view-types";
import {
  serverHostTarget,
  serverIdFromHostTargetId,
} from "../lib/server-targets";

type OverviewContainerEntry = {
  container: ManagedServer["containers"][number];
  server: ManagedServer;
  target: DeveloperTarget | undefined;
};

function serverForTargetId(
  servers: ManagedServer[],
  targetId: string,
): ManagedServer | undefined {
  const hostServerId = serverIdFromHostTargetId(targetId);
  if (hostServerId) {
    return servers.find((server) => server.id === hostServerId);
  }

  return servers.find((server) =>
    server.containers.some((container) => container.targetId === targetId),
  );
}

export function useWorkbenchInventory(
  bootstrap: AppBootstrap | null,
  activeView: ActiveView,
  visitedServerIds: string[],
) {
  const hostTargets = useMemo(
    () => bootstrap?.servers.map((server) => serverHostTarget(server)) ?? [],
    [bootstrap],
  );

  const targetById = useMemo(
    () =>
      new Map(
        [...(bootstrap?.targets ?? []), ...hostTargets].map((target) => [
          target.id,
          target,
        ]),
      ),
    [bootstrap, hostTargets],
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
    if (activeView.kind !== "container") {
      return undefined;
    }

    return targetById.get(activeView.targetId);
  }, [activeView, targetById]);

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
    hostTargets,
    standaloneTargets,
    targetById,
    visitedServers,
  };
}
