import { useEffect, useState } from "react";
import { listenTunnelStatus } from "../lib/tauri";
import type { TunnelStatus } from "../types";

function serviceKey(targetId: string, serviceId: string): string {
  return `${targetId}::${serviceId}`;
}

function tunnelKey(status: TunnelStatus): string {
  return serviceKey(status.targetId, status.serviceId || status.surfaceId);
}

export function useTunnelStatuses() {
  const [tunnelStatuses, setTunnelStatuses] = useState<Record<string, TunnelStatus>>(
    {},
  );

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listenTunnelStatus((status) => {
      if (cancelled) {
        return;
      }

      setTunnelStatuses((current) => ({
        ...current,
        [tunnelKey(status)]: status,
      }));
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
    setTunnelStatuses((current) => ({
      ...current,
      [tunnelKey(status)]: status,
    }));
  };

  const resetTunnelStatuses = () => {
    setTunnelStatuses({});
  };

  return {
    tunnelStatuses,
    setTunnelStatus,
    resetTunnelStatuses,
  };
}
