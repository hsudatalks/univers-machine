import { startTransition, useEffect, useState } from "react";
import {
  listenConnectivityStatusBatch,
  loadBootstrap,
  refreshBootstrap,
} from "../lib/tauri";
import type {
  AppBootstrap,
  ConnectivityStatusBatch,
  ConnectivityStatusEvent,
} from "../types";

function applyConnectivityStatus(
  bootstrap: AppBootstrap,
  status: ConnectivityStatusEvent,
): AppBootstrap {
  return {
    ...bootstrap,
    machines: bootstrap.machines.map((machine) => {
      if (machine.id !== status.machineId) {
        return machine;
      }

      if (status.entity === "machine") {
        return {
          ...machine,
          state: status.state,
          message: status.message,
        };
      }

      return {
        ...machine,
        containers: machine.containers.map((container) =>
          container.targetId === status.targetId
            ? {
                ...container,
                sshState: status.state,
                sshMessage: status.message,
                sshReachable: status.reachable,
              }
            : container,
        ),
      };
    }),
  };
}

function applyConnectivityStatuses(
  bootstrap: AppBootstrap,
  statuses: ConnectivityStatusBatch,
): AppBootstrap {
  return statuses.reduce(applyConnectivityStatus, bootstrap);
}

export function useWorkbenchBootstrap() {
  const [bootstrap, setBootstrap] = useState<AppBootstrap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [expandedMachineIds, setExpandedMachineIds] = useState<string[]>([]);
  const [isRefreshing, setIsRefreshing] = useState(false);

  useEffect(() => {
    let cancelled = false;

    loadBootstrap()
      .then((nextBootstrap) => {
        if (cancelled) {
          return;
        }

        setBootstrap(nextBootstrap);
        setExpandedMachineIds(nextBootstrap.machines.map((machine) => machine.id));
        setError(null);
      })
      .catch((loadError) => {
        if (cancelled) {
          return;
        }

        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load target definitions.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listenConnectivityStatusBatch((statuses) => {
      startTransition(() => {
        setBootstrap((current) =>
          current ? applyConnectivityStatuses(current, statuses) : current,
        );
      });
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  const refreshInventory = () => {
    setIsRefreshing(true);

    void refreshBootstrap()
      .then((nextBootstrap) => {
        setBootstrap(nextBootstrap);
        setExpandedMachineIds(nextBootstrap.machines.map((machine) => machine.id));
        setError(null);
      })
      .catch((loadError) => {
        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to refresh target definitions.",
        );
      })
      .finally(() => {
        setIsRefreshing(false);
      });
  };

  return {
    bootstrap,
    error,
    expandedMachineIds,
    isRefreshing,
    refreshInventory,
    setExpandedMachineIds,
  };
}
