import { useEffect, useState } from "react";
import type { DeveloperSurface, DeveloperTarget } from "../types";
import { scanLocalhostHttpServices } from "../lib/tauri";

const LOCALHOST_HOSTS = new Set(["localhost", "127.0.0.1", "::1"]);
const SCAN_INTERVAL_MS = 10_000;

export function useLocalhostServiceScan(target: DeveloperTarget): DeveloperSurface[] {
  const [discovered, setDiscovered] = useState<DeveloperSurface[]>([]);
  const isLocalhost = LOCALHOST_HOSTS.has(target.host);

  useEffect(() => {
    if (!isLocalhost) return;

    let cancelled = false;

    const scan = async () => {
      try {
        const services = await scanLocalhostHttpServices();
        if (!cancelled) {
          setDiscovered(
            services.map((svc) => ({
              id: `discovered:${svc.port}`,
              label: svc.label,
              serviceType: "http" as const,
              tunnelCommand: "",
              localUrl: svc.url,
              remoteUrl: svc.url,
            })),
          );
        }
      } catch {
        // silent — scan is best-effort
      }
    };

    void scan();
    const interval = setInterval(() => void scan(), SCAN_INTERVAL_MS);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [isLocalhost]);

  return discovered;
}
