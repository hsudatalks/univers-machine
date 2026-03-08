import { useEffect, useState } from "react";
import { loadBootstrap } from "../lib/tauri";
import type { AppBootstrap } from "../types";

export function useWorkbenchBootstrap() {
  const [bootstrap, setBootstrap] = useState<AppBootstrap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [expandedServerIds, setExpandedServerIds] = useState<string[]>([]);

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

  return {
    bootstrap,
    error,
    expandedServerIds,
    setExpandedServerIds,
  };
}
