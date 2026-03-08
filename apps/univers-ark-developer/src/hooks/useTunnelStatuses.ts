import { useEffect, useState } from "react";
import { listenTunnelStatus } from "../lib/tauri";
import type { TunnelStatus } from "../types";

function surfaceKey(targetId: string, surfaceId: string): string {
  return `${targetId}::${surfaceId}`;
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
        [surfaceKey(status.targetId, status.surfaceId)]: status,
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
      [surfaceKey(status.targetId, status.surfaceId)]: status,
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
