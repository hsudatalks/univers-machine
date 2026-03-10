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

export function warmTargetTunnels(
  target: DeveloperTarget,
  surfaceIds: string[] = defaultWarmSurfaceIds(target),
  onStatus?: (status: TunnelStatus) => void,
) {
  for (const surfaceId of surfaceIds) {
    const key = tunnelKey(target.id, surfaceId);
    desiredRegistrations.set(key, { targetId: target.id, surfaceId });
  }

  if (inflightSync) {
    void inflightSync.then((statuses) => {
      statuses?.forEach((status) => {
        onStatus?.(status);
      });
    });
    return;
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
}
