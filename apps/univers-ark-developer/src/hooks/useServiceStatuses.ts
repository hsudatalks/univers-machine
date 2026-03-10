import { useEffect, useState } from "react";
import { listenServiceStatus } from "../lib/tauri";
import type { ServiceStatus } from "../types";

function serviceKey(targetId: string, serviceId: string): string {
  return `${targetId}::${serviceId}`;
}

export function useServiceStatuses() {
  const [serviceStatuses, setServiceStatuses] = useState<
    Record<string, ServiceStatus>
  >({});

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listenServiceStatus((status) => {
      if (disposed) {
        return;
      }

      setServiceStatuses((current) => ({
        ...current,
        [serviceKey(status.targetId, status.serviceId)]: status,
      }));
    }).then((callback) => {
      unlisten = callback;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return { serviceStatuses };
}
