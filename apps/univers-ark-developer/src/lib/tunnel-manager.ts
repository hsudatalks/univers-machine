import { syncTunnelRegistrations } from "./tauri";
import type { DeveloperTarget, TunnelStatus } from "../types";

const desiredRegistrations = new Map<
  string,
  { targetId: string; surfaceId: string }
>();
let inflightSync: Promise<TunnelStatus[] | undefined> | null = null;

function tunnelKey(targetId: string, surfaceId: string): string {
  return `${targetId}::${surfaceId}`;
}

function defaultWarmSurfaceIds(target: DeveloperTarget): string[] {
  const developmentSurface = target.surfaces.find(
    (surface) => surface.id === "development",
  );

  return developmentSurface
    ? [developmentSurface.id]
    : target.surfaces[0]
      ? [target.surfaces[0].id]
      : [];
}

export function registerTunnelRequests(
  requests: Array<{ targetId: string; surfaceId: string }>,
  onStatus?: (status: TunnelStatus) => void,
) {
  for (const request of requests) {
    desiredRegistrations.set(tunnelKey(request.targetId, request.surfaceId), request);
  }

  if (inflightSync) {
    void inflightSync.then((statuses) => {
      statuses?.forEach((status) => {
        onStatus?.(status);
      });
    });
    return inflightSync;
  }

  inflightSync = syncTunnelRegistrations([...desiredRegistrations.values()])
    .then((statuses) => {
      statuses.forEach((status) => {
        onStatus?.(status);
      });
      return statuses;
    })
    .catch(() => undefined)
    .finally(() => {
      inflightSync = null;
    });

  return inflightSync;
}

export function warmTargetTunnels(
  target: DeveloperTarget,
  surfaceIds: string[] = defaultWarmSurfaceIds(target),
  onStatus?: (status: TunnelStatus) => void,
) {
  void registerTunnelRequests(
    surfaceIds.map((surfaceId) => ({ targetId: target.id, surfaceId })),
    onStatus,
  );
}
