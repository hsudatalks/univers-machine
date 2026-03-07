import { ensureTunnel } from "./tauri";
import type { DeveloperTarget, TunnelStatus } from "../types";

const inflightWarmups = new Map<string, Promise<TunnelStatus | undefined>>();

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

    const existingWarmup = inflightWarmups.get(key);

    if (existingWarmup) {
      if (onStatus) {
        void existingWarmup.then((status) => {
          if (status) {
            onStatus(status);
          }
        });
      }

      continue;
    }

    const warmup = ensureTunnel(target.id, surfaceId)
      .then((status) => {
        onStatus?.(status);
        return status;
      })
      .catch(() => undefined)
      .finally(() => {
        inflightWarmups.delete(key);
      });

    inflightWarmups.set(key, warmup);
  }
}
