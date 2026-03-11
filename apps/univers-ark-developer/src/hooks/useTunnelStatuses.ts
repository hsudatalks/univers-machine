import { startTransition, useEffect, useState } from "react";
import { listenTunnelStatusBatch } from "../lib/tauri";
import type { TunnelStatus } from "../types";

function serviceKey(targetId: string, serviceId: string): string {
  return `${targetId}::${serviceId}`;
}

function tunnelKey(status: TunnelStatus): string {
  return serviceKey(status.targetId, status.serviceId || status.surfaceId);
}

function applyTunnelStatuses(
  current: Record<string, TunnelStatus>,
  statuses: TunnelStatus[],
): Record<string, TunnelStatus> {
  if (statuses.length === 0) {
    return current;
  }

  let changed = false;
  const next = { ...current };

  for (const status of statuses) {
    const key = tunnelKey(status);
    const previous = next[key];

    if (
      previous &&
      previous.localUrl === status.localUrl &&
      previous.state === status.state &&
      previous.message === status.message
    ) {
      continue;
    }

    next[key] = status;
    changed = true;
  }

  return changed ? next : current;
}

export function useTunnelStatuses() {
  const [tunnelStatuses, setTunnelStatuses] = useState<Record<string, TunnelStatus>>(
    {},
  );

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listenTunnelStatusBatch((statuses) => {
      if (cancelled) {
        return;
      }

      startTransition(() => {
        setTunnelStatuses((current) => applyTunnelStatuses(current, statuses));
      });
    }).then((nextUnlisten) => {
      if (cancelled) {
        nextUnlisten();
        return;
      }

      unlisten = nextUnlisten;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const setTunnelStatus = (status: TunnelStatus) => {
    startTransition(() => {
      setTunnelStatuses((current) => applyTunnelStatuses(current, [status]));
    });
  };

  const setTunnelStatusesBatch = (statuses: TunnelStatus[]) => {
    startTransition(() => {
      setTunnelStatuses((current) => applyTunnelStatuses(current, statuses));
    });
  };

  const resetTunnelStatuses = () => {
    setTunnelStatuses({});
  };

  return {
    tunnelStatuses,
    setTunnelStatus,
    setTunnelStatusesBatch,
    resetTunnelStatuses,
  };
}
