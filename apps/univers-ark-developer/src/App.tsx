import { useEffect, useEffectEvent, useMemo, useState } from "react";
import { TerminalPane } from "./components/TerminalPane";
import "./App.css";
import {
  ensureTunnel,
  listenTunnelStatus,
  loadBootstrap,
  restartTunnel,
} from "./lib/tauri";
import type {
  AppBootstrap,
  DeveloperSurface,
  DeveloperTarget,
  TunnelStatus,
} from "./types";

interface CachedBrowserSurface {
  cacheKey: string;
  target: DeveloperTarget;
  surface: DeveloperSurface;
}

function surfaceKey(targetId: string, surfaceId: string): string {
  return `${targetId}::${surfaceId}`;
}

function splitSurfaceKey(cacheKey: string): [string, string] | null {
  const separatorIndex = cacheKey.indexOf("::");

  if (separatorIndex === -1) {
    return null;
  }

  return [cacheKey.slice(0, separatorIndex), cacheKey.slice(separatorIndex + 2)];
}

function preferredSurfaceId(target: DeveloperTarget): string {
  return target.surfaces.find((surface) => surface.id === "preview")?.id ??
    target.surfaces[0]?.id ??
    "";
}

function App() {
  const [bootstrap, setBootstrap] = useState<AppBootstrap | null>(null);
  const [selectedTargetId, setSelectedTargetId] = useState("");
  const [selectedSurfaceId, setSelectedSurfaceId] = useState("preview");
  const [frameVersions, setFrameVersions] = useState<Record<string, number>>({});
  const [visitedSurfaceKeys, setVisitedSurfaceKeys] = useState<string[]>([]);
  const [tunnelStatuses, setTunnelStatuses] = useState<Record<string, TunnelStatus>>(
    {},
  );
  const [error, setError] = useState<string | null>(null);

  const cacheBrowserSurface = (
    targetId: string,
    surfaceId: string,
    options?: { reloadPreview?: boolean },
  ) => {
    if (!targetId || !surfaceId) {
      return;
    }

    const cacheKey = surfaceKey(targetId, surfaceId);

    setVisitedSurfaceKeys((current) =>
      current.includes(cacheKey) ? current : [...current, cacheKey],
    );

    setFrameVersions((current) =>
      options?.reloadPreview
        ? { ...current, [cacheKey]: (current[cacheKey] ?? 0) + 1 }
        : cacheKey in current
          ? current
          : { ...current, [cacheKey]: 0 },
    );
  };

  const commitTunnelStatus = (
    status: TunnelStatus,
    options?: { reloadPreview?: boolean },
  ) => {
    const cacheKey = surfaceKey(status.targetId, status.surfaceId);

    setTunnelStatuses((current) => ({ ...current, [cacheKey]: status }));

    if (status.state === "direct" || status.state === "running") {
      cacheBrowserSurface(status.targetId, status.surfaceId, options);
    }
  };

  const applyTunnelStatusEvent = useEffectEvent((status: TunnelStatus) => {
    commitTunnelStatus(status, { reloadPreview: status.state === "running" });
  });

  const commitTunnelStatusFromEffect = useEffectEvent((status: TunnelStatus) => {
    commitTunnelStatus(status);
  });

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void listenTunnelStatus((status) => {
      applyTunnelStatusEvent(status);
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

  useEffect(() => {
    let cancelled = false;

    loadBootstrap()
      .then((nextBootstrap) => {
        if (cancelled) {
          return;
        }

        const initialTarget =
          nextBootstrap.targets.find(
            (target) => target.id === nextBootstrap.selectedTargetId,
          ) ?? nextBootstrap.targets[0];

        setBootstrap(nextBootstrap);
        setSelectedTargetId(initialTarget?.id ?? "");
        setSelectedSurfaceId(initialTarget ? preferredSurfaceId(initialTarget) : "");
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

  const selectedTarget = useMemo<DeveloperTarget | undefined>(
    () => bootstrap?.targets.find((target) => target.id === selectedTargetId),
    [bootstrap, selectedTargetId],
  );

  useEffect(() => {
    if (!selectedTarget) {
      return undefined;
    }

    let cancelled = false;

    for (const surface of selectedTarget.surfaces) {
      void ensureTunnel(selectedTarget.id, surface.id)
        .then((status) => {
          if (cancelled) {
            return;
          }

          commitTunnelStatusFromEffect(status);
        })
        .catch((tunnelError) => {
          if (cancelled) {
            return;
          }

          commitTunnelStatusFromEffect({
            targetId: selectedTarget.id,
            surfaceId: surface.id,
            state: "error",
            message:
              tunnelError instanceof Error
                ? tunnelError.message
                : `Failed to prepare the ${surface.label.toLowerCase()} tunnel.`,
          });
        });
    }

    return () => {
      cancelled = true;
    };
  }, [selectedTarget]);

  const selectedSurface =
    selectedTarget?.surfaces.find((surface) => surface.id === selectedSurfaceId) ??
    (selectedTarget ? selectedTarget.surfaces.find((surface) => surface.id === preferredSurfaceId(selectedTarget)) : undefined) ??
    selectedTarget?.surfaces[0];

  const cachedBrowserSurfaces = useMemo<CachedBrowserSurface[]>(() => {
    if (!bootstrap) {
      return [];
    }

    return visitedSurfaceKeys
      .map((cacheKey) => {
        const parts = splitSurfaceKey(cacheKey);

        if (!parts) {
          return null;
        }

        const [targetId, surfaceId] = parts;
        const target = bootstrap.targets.find((entry) => entry.id === targetId);
        const surface = target?.surfaces.find((entry) => entry.id === surfaceId);

        if (!target || !surface) {
          return null;
        }

        return { cacheKey, target, surface };
      })
      .filter((entry): entry is CachedBrowserSurface => entry !== null);
  }, [bootstrap, visitedSurfaceKeys]);

  if (error) {
    return (
      <main className="shell shell-state">
        <section className="state-panel">
          <span className="state-label">Error</span>
          <p className="state-copy">{error}</p>
        </section>
      </main>
    );
  }

  if (!bootstrap || !selectedTarget || !selectedSurface) {
    return (
      <main className="shell shell-state">
        <section className="state-panel">
          <span className="state-label">Loading</span>
          <p className="state-copy">Preparing target definitions.</p>
        </section>
      </main>
    );
  }

  const selectedSurfaceCacheKey = surfaceKey(selectedTarget.id, selectedSurface.id);
  const selectedTunnelStatus =
    tunnelStatuses[selectedSurfaceCacheKey] ??
    (selectedSurface.tunnelCommand
      ? {
          targetId: selectedTarget.id,
          surfaceId: selectedSurface.id,
          state: "starting",
          message: `Preparing the ${selectedSurface.label.toLowerCase()} tunnel.`,
        }
      : {
          targetId: selectedTarget.id,
          surfaceId: selectedSurface.id,
          state: "direct",
          message: `${selectedSurface.label} is using the local URL directly.`,
        });

  const tunnelStatusLabel =
    {
      direct: "Direct",
      starting: "Starting",
      running: "Running",
      stopped: "Stopped",
      error: "Error",
    }[selectedTunnelStatus.state] ?? selectedTunnelStatus.state;

  const isSelectedBrowserCached = visitedSurfaceKeys.includes(selectedSurfaceCacheKey);
  const showBrowserOverlay =
    !isSelectedBrowserCached ||
    selectedTunnelStatus.state === "starting" ||
    selectedTunnelStatus.state === "stopped" ||
    selectedTunnelStatus.state === "error";

  return (
    <main className="shell">
      <section className="switcher" aria-label="Container switcher">
        {bootstrap.targets.map((target) => {
          const isActive = target.id === selectedTarget.id;

          return (
            <button
              key={target.id}
              className={`switcher-button ${isActive ? "is-active" : ""}`}
              onClick={() => setSelectedTargetId(target.id)}
              type="button"
            >
              <span className="switcher-label">{target.label}</span>
              <span className="switcher-host">{target.host}</span>
            </button>
          );
        })}
      </section>

      <section className="workspace">
        <article className="panel terminal-panel">
          <TerminalPane key={selectedTarget.id} target={selectedTarget} />
        </article>

        <article className="panel browser-panel">
          <header className="panel-header browser-header">
            <div className="browser-heading">
              <span className="panel-title">Browser</span>
              <div className="surface-tabs" role="tablist" aria-label="Browser surfaces">
                {selectedTarget.surfaces.map((surface) => {
                  const isActive = surface.id === selectedSurface.id;

                  return (
                    <button
                      key={surface.id}
                      className={`surface-tab ${isActive ? "is-active" : ""}`}
                      onClick={() => setSelectedSurfaceId(surface.id)}
                      role="tab"
                      type="button"
                    >
                      {surface.label}
                    </button>
                  );
                })}
              </div>
            </div>

            <div className="browser-bar">
              <span
                className={`terminal-status status-${selectedTunnelStatus.state}`}
              >
                {tunnelStatusLabel}
              </span>
              <code className="browser-url">{selectedSurface.localUrl}</code>
              <button
                className="panel-button"
                disabled={!selectedSurface.tunnelCommand}
                onClick={() => {
                  void restartTunnel(selectedTarget.id, selectedSurface.id)
                    .then((status) => {
                      commitTunnelStatus(status);
                    })
                    .catch((tunnelError) => {
                      commitTunnelStatus({
                        targetId: selectedTarget.id,
                        surfaceId: selectedSurface.id,
                        state: "error",
                        message:
                          tunnelError instanceof Error
                            ? tunnelError.message
                            : `Failed to restart the ${selectedSurface.label.toLowerCase()} tunnel.`,
                      });
                    });
                }}
                type="button"
              >
                Restart Tunnel
              </button>
              <button
                className="panel-button"
                onClick={() =>
                  setFrameVersions((current) => ({
                    ...current,
                    [selectedSurfaceCacheKey]:
                      (current[selectedSurfaceCacheKey] ?? 0) + 1,
                  }))
                }
                type="button"
              >
                Reload
              </button>
              <a
                className="panel-button panel-link"
                href={selectedSurface.localUrl}
                rel="noreferrer"
                target="_blank"
              >
                Open
              </a>
            </div>
          </header>

          <div className="browser-stage">
            {cachedBrowserSurfaces.map(({ cacheKey, surface, target }) => {
              const isActive = cacheKey === selectedSurfaceCacheKey;

              return (
                <iframe
                  className={`browser-frame ${isActive ? "is-active" : ""}`}
                  key={`${cacheKey}-${frameVersions[cacheKey] ?? 0}`}
                  src={surface.localUrl}
                  title={`${target.label} ${surface.label}`}
                />
              );
            })}

            {showBrowserOverlay ? (
              <div className="browser-overlay">
                <div className="browser-placeholder">
                  <span className="state-label">{selectedSurface.label}</span>
                  <p className="browser-placeholder-title">{tunnelStatusLabel}</p>
                  <p className="browser-placeholder-copy">
                    {selectedTunnelStatus.message}
                  </p>
                </div>
              </div>
            ) : null}
          </div>
        </article>
      </section>
    </main>
  );
}

export default App;
