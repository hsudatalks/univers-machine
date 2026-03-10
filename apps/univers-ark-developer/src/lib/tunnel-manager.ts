import { syncTunnelRegistrations } from "./tauri";
import type { DeveloperTarget, TunnelStatus } from "../types";
import { primaryBrowserService } from "./target-services";

const desiredRegistrations = new Map<
  string,
  { targetId: string; serviceId: string }
>();
let inflightSync: Promise<TunnelStatus[] | undefined> | null = null;

function tunnelKey(targetId: string, serviceId: string): string {
  return `${targetId}::${serviceId}`;
}

function defaultWarmServiceIds(target: DeveloperTarget): string[] {
  const primaryService = primaryBrowserService(target);
  return primaryService ? [primaryService.id] : [];
}

export function registerTunnelRequests(
  requests: Array<{ targetId: string; serviceId: string }>,
  onStatus?: (status: TunnelStatus) => void,
) {
  for (const request of requests) {
    desiredRegistrations.set(tunnelKey(request.targetId, request.serviceId), request);
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
  serviceIds: string[] = defaultWarmServiceIds(target),
  onStatus?: (status: TunnelStatus) => void,
) {
  void registerTunnelRequests(
    serviceIds.map((serviceId) => ({ targetId: target.id, serviceId })),
    onStatus,
  );
}
