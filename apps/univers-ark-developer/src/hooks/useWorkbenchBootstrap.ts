import { useEffect, useState } from "react";
import { loadBootstrap, refreshBootstrap } from "../lib/tauri";
import type { AppBootstrap } from "../types";

export function useWorkbenchBootstrap() {
  const [bootstrap, setBootstrap] = useState<AppBootstrap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [expandedServerIds, setExpandedServerIds] = useState<string[]>([]);
  const [isRefreshing, setIsRefreshing] = useState(false);

  useEffect(() => {
    let cancelled = false;

    loadBootstrap()
      .then((nextBootstrap) => {
        if (cancelled) {
          return;
        }

        setBootstrap(nextBootstrap);
        setExpandedServerIds(nextBootstrap.servers.map((server) => server.id));
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

  const refreshInventory = () => {
    setIsRefreshing(true);

    void refreshBootstrap()
      .then((nextBootstrap) => {
        setBootstrap(nextBootstrap);
        setExpandedServerIds(nextBootstrap.servers.map((server) => server.id));
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
    expandedServerIds,
    isRefreshing,
    refreshInventory,
    setExpandedServerIds,
  };
}
