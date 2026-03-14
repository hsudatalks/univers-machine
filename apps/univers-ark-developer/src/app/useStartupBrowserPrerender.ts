import { useEffect, useMemo, useRef, useState } from "react";
import { preloadBrowserFrames } from "../lib/browser-cache";
import { registerTunnelRequests } from "../lib/tunnel-manager";
import { backgroundPrerenderBrowserServices } from "../lib/target-services";
import type { AppBootstrap, DeveloperTarget, TunnelStatus } from "../types";

const STARTUP_PRERENDER_INITIAL_BATCH = 2;
const STARTUP_PRERENDER_BATCH_SIZE = 2;
const STARTUP_PRERENDER_BATCH_INTERVAL_MS = 1500;

interface StartupPrerenderDescriptor {
  cacheKey: string;
  serviceId: string;
  surface: {
    id: string;
    label: string;
    localUrl: string;
  };
  target: DeveloperTarget;
}

function surfaceKey(targetId: string, surfaceId: string): string {
  return `${targetId}::${surfaceId}`;
}

function isReadyTunnelState(state: string | undefined): boolean {
  return state === "direct" || state === "running";
}

interface UseStartupBrowserPrerenderOptions {
  bootstrap: AppBootstrap | null;
  isDocumentVisible: boolean;
  isNetworkOnline: boolean;
  onTunnelStatus: (status: TunnelStatus) => void;
  tunnelStatuses: Record<string, TunnelStatus>;
}

export function useStartupBrowserPrerender({
  bootstrap,
  isDocumentVisible,
  isNetworkOnline,
  onTunnelStatus,
  tunnelStatuses,
}: UseStartupBrowserPrerenderOptions) {
  const startupPrerenderDescriptors = useMemo<StartupPrerenderDescriptor[]>(
    () =>
      bootstrap
        ? bootstrap.targets
            .filter(
              (target) =>
                !bootstrap.machines.some(
                  (machine) => machine.hostTargetId === target.id,
                ),
            )
            .flatMap((target) =>
              backgroundPrerenderBrowserServices(target).map((service) => ({
                cacheKey: surfaceKey(target.id, service.id),
                serviceId: service.id,
                surface: service.web,
                target,
              })),
            )
        : [],
    [bootstrap],
  );
  const [startupPrerenderBudget, setStartupPrerenderBudget] = useState(0);
  const [startupPrerenderVersions, setStartupPrerenderVersions] = useState<
    Record<string, number>
  >({});
  const previousStartupPrerenderStatesRef = useRef<
    Record<string, string | undefined>
  >({});
  const activeStartupPrerenderDescriptors = useMemo(
    () => startupPrerenderDescriptors.slice(0, startupPrerenderBudget),
    [startupPrerenderBudget, startupPrerenderDescriptors],
  );

  useEffect(() => {
    if (startupPrerenderDescriptors.length === 0) {
      setStartupPrerenderBudget(0);
      return;
    }

    if (!isDocumentVisible || !isNetworkOnline) {
      return;
    }

    const initialBudget = Math.min(
      STARTUP_PRERENDER_INITIAL_BATCH,
      startupPrerenderDescriptors.length,
    );

    if (startupPrerenderBudget === 0) {
      setStartupPrerenderBudget(initialBudget);
      return;
    }

    if (startupPrerenderBudget >= startupPrerenderDescriptors.length) {
      return;
    }

    const timeout = window.setTimeout(() => {
      setStartupPrerenderBudget((current) =>
        Math.min(
          startupPrerenderDescriptors.length,
          Math.max(initialBudget, current + STARTUP_PRERENDER_BATCH_SIZE),
        ),
      );
    }, STARTUP_PRERENDER_BATCH_INTERVAL_MS);

    return () => {
      window.clearTimeout(timeout);
    };
  }, [
    isDocumentVisible,
    isNetworkOnline,
    startupPrerenderBudget,
    startupPrerenderDescriptors.length,
  ]);

  useEffect(() => {
    if (activeStartupPrerenderDescriptors.length === 0) {
      return;
    }

    void registerTunnelRequests(
      activeStartupPrerenderDescriptors.map(({ target, serviceId }) => ({
        targetId: target.id,
        serviceId,
      })),
      onTunnelStatus,
    );
  }, [activeStartupPrerenderDescriptors, onTunnelStatus]);

  useEffect(() => {
    const previousStates = previousStartupPrerenderStatesRef.current;
    const nextReadyKeys: string[] = [];

    for (const descriptor of activeStartupPrerenderDescriptors) {
      const nextState = tunnelStatuses[descriptor.cacheKey]?.state;
      const previousState = previousStates[descriptor.cacheKey];

      if (!isReadyTunnelState(previousState) && isReadyTunnelState(nextState)) {
        nextReadyKeys.push(descriptor.cacheKey);
      }

      previousStates[descriptor.cacheKey] = nextState;
    }

    if (nextReadyKeys.length === 0) {
      return;
    }

    setStartupPrerenderVersions((current) => {
      const next = { ...current };

      for (const key of nextReadyKeys) {
        next[key] = (next[key] ?? 0) + 1;
      }

      return next;
    });
  }, [activeStartupPrerenderDescriptors, tunnelStatuses]);

  useEffect(() => {
    if (
      !isDocumentVisible ||
      !isNetworkOnline ||
      activeStartupPrerenderDescriptors.length === 0
    ) {
      return;
    }

    preloadBrowserFrames(
      activeStartupPrerenderDescriptors
        .filter(({ cacheKey, target }) => {
          const state = tunnelStatuses[cacheKey]?.state;
          return isReadyTunnelState(state) || target.transport === "local";
        })
        .map(({ cacheKey, surface, target }) => ({
          cacheKey,
          frameVersion: startupPrerenderVersions[cacheKey] ?? 0,
          src: tunnelStatuses[cacheKey]?.localUrl ?? surface.localUrl,
          title: `${target.label} ${surface.label}`,
        })),
    );
  }, [
    activeStartupPrerenderDescriptors,
    isDocumentVisible,
    isNetworkOnline,
    startupPrerenderVersions,
    tunnelStatuses,
  ]);
}
