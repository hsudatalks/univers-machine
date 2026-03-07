import { useCallback, useEffect, useMemo, useState } from "react";
import {
  BrowserPane,
  type BrowserFrameInstance,
} from "./components/BrowserPane";
import { SidebarNav } from "./components/SidebarNav";
import { TerminalCard } from "./components/TerminalCard";
import { TerminalPane } from "./components/TerminalPane";
import "./App.css";
import { pruneBrowserFrames } from "./lib/browser-cache";
import {
  listenTunnelStatus,
  loadBootstrap,
  refreshBootstrap,
  restartTunnel,
} from "./lib/tauri";
import { warmTargetTunnels } from "./lib/tunnel-manager";
import type {
  AppBootstrap,
  DeveloperSurface,
  DeveloperTarget,
  ManagedContainer,
  ManagedServer,
  TunnelStatus,
} from "./types";

type ActiveView =
  | { kind: "overview" }
  | { kind: "server"; serverId: string }
  | { kind: "container"; targetId: string };

function surfaceKey(targetId: string, surfaceId: string): string {
  return `${targetId}::${surfaceId}`;
}

function defaultBrowserSurface(target: DeveloperTarget): DeveloperSurface | undefined {
  return (
    target.surfaces.find((surface) => surface.id === "development") ??
    target.surfaces[0]
  );
}

function fallbackTunnelStatus(
  targetId: string,
  surface: DeveloperSurface,
): TunnelStatus {
  return surface.tunnelCommand
    ? {
        targetId,
        surfaceId: surface.id,
        state: "starting",
        message: `Warming the ${surface.label.toLowerCase()} tunnel in the background.`,
      }
    : {
        targetId,
        surfaceId: surface.id,
        state: "direct",
        message: `${surface.label} is using the local URL directly.`,
      };
}

function resolvePreferredTarget(
  bootstrap: AppBootstrap,
  preferredTargetId?: string,
): DeveloperTarget | undefined {
  if (preferredTargetId) {
    const preferredTarget = bootstrap.targets.find(
      (target) => target.id === preferredTargetId,
    );

    if (preferredTarget) {
      return preferredTarget;
    }
  }

  return (
    bootstrap.targets.find(
      (target) => target.id === bootstrap.selectedTargetId,
    ) ?? bootstrap.targets[0]
  );
}

function uniqueStrings(values: string[]): string[] {
  const seen = new Set<string>();

  return values.filter((value) => {
    if (seen.has(value)) {
      return false;
    }

    seen.add(value);
    return true;
  });
}

function normalizeActiveView(
  bootstrap: AppBootstrap,
  view: ActiveView,
): ActiveView {
  if (view.kind === "overview") {
    return view;
  }

  if (view.kind === "server") {
    return bootstrap.servers.some((server) => server.id === view.serverId)
      ? view
      : { kind: "overview" };
  }

  return bootstrap.targets.some((target) => target.id === view.targetId)
    ? view
    : { kind: "overview" };
}

function serverForTargetId(
  servers: ManagedServer[],
  targetId: string,
): ManagedServer | undefined {
  return servers.find((server) =>
    server.containers.some((container) => container.targetId === targetId),
  );
}

function containerSubtitle(
  server: ManagedServer | undefined,
  container: ManagedContainer,
): string {
  const segments = [server?.label, container.ipv4 || container.sshDestination].filter(
    Boolean,
  );

  return segments.join(" · ");
}

function App() {
  const [bootstrap, setBootstrap] = useState<AppBootstrap | null>(null);
  const [activeView, setActiveView] = useState<ActiveView>({ kind: "overview" });
  const [visitedContainerIds, setVisitedContainerIds] = useState<string[]>([]);
  const [visitedServerIds, setVisitedServerIds] = useState<string[]>([]);
  const [selectedTargetId, setSelectedTargetId] = useState("");
  const [browserFrameVersions, setBrowserFrameVersions] = useState<
    Record<string, number>
  >({});
  const [loadedBrowserKeys, setLoadedBrowserKeys] = useState<string[]>([]);
  const [tunnelStatuses, setTunnelStatuses] = useState<Record<string, TunnelStatus>>(
    {},
  );
  const [expandedServerIds, setExpandedServerIds] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isRefreshingInventory, setIsRefreshingInventory] = useState(false);

  const recordTunnelStatus = useCallback(
    (
      status: TunnelStatus,
      options?: { reloadFrame?: boolean; unloadFrame?: boolean },
    ) => {
      const cacheKey = surfaceKey(status.targetId, status.surfaceId);

      setTunnelStatuses((current) => ({ ...current, [cacheKey]: status }));

      if (options?.unloadFrame || status.state === "error" || status.state === "stopped") {
        setLoadedBrowserKeys((current) =>
          current.filter((entry) => entry !== cacheKey),
        );
      }

      if (status.state === "direct" || status.state === "running") {
        setLoadedBrowserKeys((current) =>
          current.includes(cacheKey) ? current : [...current, cacheKey],
        );
        setBrowserFrameVersions((current) =>
          options?.reloadFrame
            ? { ...current, [cacheKey]: (current[cacheKey] ?? 0) + 1 }
            : cacheKey in current
              ? current
              : { ...current, [cacheKey]: 0 },
        );
      }
    },
    [],
  );

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void listenTunnelStatus((status) => {
      recordTunnelStatus(status);
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
  }, [recordTunnelStatus]);

  useEffect(() => {
    let cancelled = false;

    loadBootstrap()
      .then((nextBootstrap) => {
        if (cancelled) {
          return;
        }

        const initialTarget = resolvePreferredTarget(nextBootstrap);

        setBootstrap(nextBootstrap);
        setActiveView({ kind: "overview" });
        setVisitedContainerIds([]);
        setVisitedServerIds([]);
        setSelectedTargetId(initialTarget?.id ?? "");
        setBrowserFrameVersions({});
        setLoadedBrowserKeys([]);
        setTunnelStatuses({});
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

  const targetById = useMemo(
    () => new Map(bootstrap?.targets.map((target) => [target.id, target]) ?? []),
    [bootstrap],
  );

  const managedTargetIds = useMemo(
    () =>
      new Set(
        bootstrap?.servers.flatMap((server) =>
          server.containers.map((container) => container.targetId),
        ) ?? [],
      ),
    [bootstrap],
  );

  const standaloneTargets = useMemo(
    () =>
      bootstrap?.targets.filter((target) => !managedTargetIds.has(target.id)) ?? [],
    [bootstrap, managedTargetIds],
  );

  const overviewContainers = useMemo(
    () =>
      bootstrap?.servers.flatMap((server) =>
        server.containers.map((container) => ({
          container,
          server,
          target: targetById.get(container.targetId),
        })),
      ) ?? [],
    [bootstrap, targetById],
  );

  const visitedServers = useMemo(
    () =>
      visitedServerIds
        .map((serverId) => bootstrap?.servers.find((server) => server.id === serverId))
        .filter((server): server is ManagedServer => Boolean(server)),
    [bootstrap, visitedServerIds],
  );

  const activeContainerTarget = useMemo(() => {
    if (!bootstrap || activeView.kind !== "container") {
      return undefined;
    }

    return bootstrap.targets.find((target) => target.id === activeView.targetId);
  }, [activeView, bootstrap]);

  const activeContainerServer = useMemo(
    () =>
      activeContainerTarget
        ? serverForTargetId(bootstrap?.servers ?? [], activeContainerTarget.id)
        : undefined,
    [activeContainerTarget, bootstrap],
  );

  const browserFrameForTarget = (target: DeveloperTarget): BrowserFrameInstance | undefined => {
    const surface = defaultBrowserSurface(target);

    if (!surface) {
      return undefined;
    }

    const cacheKey = surfaceKey(target.id, surface.id);

    return {
      cacheKey,
      frameVersion: browserFrameVersions[cacheKey] ?? 0,
      isActive: true,
      isLoaded: loadedBrowserKeys.includes(cacheKey),
      status: tunnelStatuses[cacheKey] ?? fallbackTunnelStatus(target.id, surface),
      surface,
      target,
    };
  };

  const reloadBrowserSurface = (targetId: string, surfaceId: string) => {
    const cacheKey = surfaceKey(targetId, surfaceId);

    setLoadedBrowserKeys((current) =>
      current.includes(cacheKey) ? current : [...current, cacheKey],
    );
    setBrowserFrameVersions((current) => ({
      ...current,
      [cacheKey]: (current[cacheKey] ?? 0) + 1,
    }));
  };

  const restartBrowserSurface = (target: DeveloperTarget, surface: DeveloperSurface) => {
    const cacheKey = surfaceKey(target.id, surface.id);

    setLoadedBrowserKeys((current) => current.filter((entry) => entry !== cacheKey));
    setBrowserFrameVersions((current) => ({
      ...current,
      [cacheKey]: (current[cacheKey] ?? 0) + 1,
    }));

    void restartTunnel(target.id, surface.id)
      .then((status) => {
        recordTunnelStatus(status, { unloadFrame: status.state === "starting" });
      })
      .catch((restartError) => {
        recordTunnelStatus({
          targetId: target.id,
          surfaceId: surface.id,
          state: "error",
          message:
            restartError instanceof Error
              ? restartError.message
              : `Failed to restart the ${surface.label.toLowerCase()} tunnel.`,
        });
      });
  };

  const setContainerView = (targetId: string) => {
    const nextTarget = targetById.get(targetId);

    if (!nextTarget) {
      return;
    }

    setSelectedTargetId(targetId);
    setActiveView({ kind: "container", targetId });
    setVisitedContainerIds((current) => uniqueStrings([...current, targetId]));
    warmTargetTunnels(nextTarget, undefined, (status) => {
      recordTunnelStatus(status);
    });
  };

  const refreshInventory = () => {
    setIsRefreshingInventory(true);

    void refreshBootstrap()
      .then((nextBootstrap) => {
        const nextTarget = resolvePreferredTarget(nextBootstrap, selectedTargetId);
        const validBrowserKeys = new Set(
          nextBootstrap.targets.flatMap((target) =>
            target.surfaces.map((surface) => surfaceKey(target.id, surface.id)),
          ),
        );

        setBootstrap(nextBootstrap);
        setSelectedTargetId(nextTarget?.id ?? "");
        setExpandedServerIds((current) =>
          uniqueStrings([...current, ...nextBootstrap.servers.map((server) => server.id)]),
        );
        setVisitedContainerIds((current) =>
          current.filter((targetId) =>
            nextBootstrap.targets.some((target) => target.id === targetId),
          ),
        );
        setVisitedServerIds((current) =>
          current.filter((serverId) =>
            nextBootstrap.servers.some((server) => server.id === serverId),
          ),
        );
        setBrowserFrameVersions((current) =>
          Object.fromEntries(
            Object.entries(current).filter(([cacheKey]) => validBrowserKeys.has(cacheKey)),
          ),
        );
        setLoadedBrowserKeys((current) =>
          current.filter((cacheKey) => validBrowserKeys.has(cacheKey)),
        );
        setTunnelStatuses((current) =>
          Object.fromEntries(
            Object.entries(current).filter(([cacheKey]) => validBrowserKeys.has(cacheKey)),
          ),
        );
        pruneBrowserFrames([...validBrowserKeys]);
        setActiveView((current) => normalizeActiveView(nextBootstrap, current));
        setError(null);
      })
      .catch((refreshError) => {
        setError(
          refreshError instanceof Error
            ? refreshError.message
            : "Failed to refresh target inventory.",
        );
      })
      .finally(() => {
        setIsRefreshingInventory(false);
      });
  };

  const toggleServerExpansion = (serverId: string) => {
    setExpandedServerIds((current) =>
      current.includes(serverId)
        ? current.filter((entry) => entry !== serverId)
        : [...current, serverId],
    );
  };

  const renderUnavailableTerminalCard = (
    container: ManagedContainer,
    server?: ManagedServer,
  ) => (
    <article className="panel terminal-card terminal-card-unavailable" key={container.targetId}>
      <header className="panel-header terminal-placeholder-header">
        <div className="terminal-copy">
          <span className="panel-title">{container.label}</span>
          <span className="panel-meta">{containerSubtitle(server, container)}</span>
        </div>

        <div className="terminal-meta">
          <span className="terminal-status status-error">{container.sshState}</span>
        </div>
      </header>

      <div className="terminal-placeholder-body">
        <p className="terminal-placeholder-copy">{container.sshMessage}</p>
      </div>
    </article>
  );

  const renderTerminalCard = (
    target: DeveloperTarget,
    options?: {
      key?: string;
      meta?: string;
      pageVisible?: boolean;
      title?: string;
    },
  ) => (
    <TerminalCard
      key={options?.key ?? target.id}
      meta={options?.meta}
      onOpenWorkspace={() => setContainerView(target.id)}
      pageVisible={options?.pageVisible}
      target={target}
      title={options?.title ?? target.label}
    />
  );

  const renderOverviewPage = (pageVisible: boolean) => {
    const reachableContainers = overviewContainers.filter(
      (entry) => entry.container.sshReachable,
    ).length;

    return (
      <>
        <header className="content-header">
          <div className="content-header-copy">
            <span className="panel-title">Overview</span>
            <h1 className="content-title">All Containers</h1>
          </div>

          <div className="content-meta-row">
            <span className="content-chip">
              {bootstrap?.servers.length ?? 0} server(s)
            </span>
            <span className="content-chip">
              {overviewContainers.length} container(s)
            </span>
            <span className="content-chip">{reachableContainers} SSH ready</span>
          </div>
        </header>

        <section className="page-section">
          <div className="terminal-grid">
            {overviewContainers.map(({ container, server, target }) =>
              target
                ? renderTerminalCard(target, {
                    key: container.targetId,
                    meta: containerSubtitle(server, container),
                    pageVisible,
                    title: container.label,
                  })
                : renderUnavailableTerminalCard(container, server),
            )}

            {standaloneTargets.map((target) =>
              renderTerminalCard(target, {
                key: target.id,
                meta: target.host,
                pageVisible,
                title: target.label,
              }),
            )}
          </div>
        </section>
      </>
    );
  };

  const renderServerPage = (server: ManagedServer, pageVisible: boolean) => {
    const reachableContainers = server.containers.filter(
      (container) => container.sshReachable,
    ).length;

    return (
      <>
        <header className="content-header">
          <div className="content-header-copy">
            <span className="panel-title">Server</span>
            <h1 className="content-title">{server.label}</h1>
          </div>

          <div className="content-meta-row">
            <span className="content-chip">{server.host}</span>
            <span className="content-chip">
              {server.containers.length} container(s)
            </span>
            <span className="content-chip">{reachableContainers} SSH ready</span>
          </div>
        </header>

        <section className="page-section">
          <div className="terminal-grid">
            {server.containers.map((container) => {
              const target = targetById.get(container.targetId);

              return target
                ? renderTerminalCard(target, {
                    key: container.targetId,
                    meta: containerSubtitle(server, container),
                    pageVisible,
                    title: container.label,
                  })
                : renderUnavailableTerminalCard(container, server);
            })}
          </div>
        </section>
      </>
    );
  };

  const renderContainerPage = (target: DeveloperTarget, pageVisible: boolean) => {
    const containerServer = serverForTargetId(bootstrap?.servers ?? [], target.id);
    const activeBrowserFrame = browserFrameForTarget(target);

    return (
      <>
        <header className="content-header">
          <div className="content-header-copy">
            <span className="panel-title">Container</span>
            <h1 className="content-title">{target.label}</h1>
          </div>

          <div className="content-meta-row">
            {containerServer ? (
              <span className="content-chip">{containerServer.label}</span>
            ) : null}
            <span className="content-chip">{target.host}</span>
            {activeBrowserFrame ? (
              <span className="content-chip">{activeBrowserFrame.surface.label}</span>
            ) : null}
          </div>
        </header>

        {pageVisible ? (
          <section className="workspace">
            <article className="panel terminal-panel">
              <TerminalPane target={target} />
            </article>

            <section className="browser-workspace">
              <header className="browser-workspace-header">
                <div className="browser-workspace-copy">
                  <span className="panel-title">Browser</span>
                  <p className="panel-description">
                    Browser attach is passive. The tunnel warms in the background.
                  </p>
                </div>
              </header>

              {activeBrowserFrame ? (
                <div className="browser-grid">
                  <BrowserPane
                    activeFrame={activeBrowserFrame}
                    isKeepAlive={false}
                    onReload={() => {
                      reloadBrowserSurface(
                        activeBrowserFrame.target.id,
                        activeBrowserFrame.surface.id,
                      );
                    }}
                    onRestart={() => {
                      restartBrowserSurface(
                        activeBrowserFrame.target,
                        activeBrowserFrame.surface,
                      );
                    }}
                    onToggleKeepAlive={() => undefined}
                    retainedFrames={
                      activeBrowserFrame.isLoaded ? [activeBrowserFrame] : []
                    }
                    showKeepAlive={false}
                    slotLabel={activeBrowserFrame.surface.label}
                  />
                </div>
              ) : (
                <section className="state-panel browser-empty-state">
                  <span className="state-label">No Browser Surface</span>
                  <p className="state-copy">
                    The current container does not expose an attachable browser surface.
                  </p>
                </section>
              )}
            </section>
          </section>
        ) : null}
      </>
    );
  };

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

  if (!bootstrap) {
    return (
      <main className="shell shell-state">
        <section className="state-panel">
          <span className="state-label">Loading</span>
          <p className="state-copy">Preparing target definitions.</p>
        </section>
      </main>
    );
  }

  return (
    <main className="shell shell-layout">
      <SidebarNav
        activeServerId={
          activeView.kind === "server"
            ? activeView.serverId
            : activeView.kind === "container"
              ? activeContainerServer?.id
              : undefined
        }
        activeTargetId={activeView.kind === "container" ? activeView.targetId : undefined}
        availableTargetIds={bootstrap.targets.map((target) => target.id)}
        bootstrap={bootstrap}
        expandedServerIds={expandedServerIds}
        isOverviewActive={activeView.kind === "overview"}
        isRefreshing={isRefreshingInventory}
        onRefresh={refreshInventory}
        onSelectContainer={setContainerView}
        onSelectOverview={() => {
          setActiveView({ kind: "overview" });
        }}
        onSelectServer={(serverId) => {
          setActiveView({ kind: "server", serverId });
          setVisitedServerIds((current) => uniqueStrings([...current, serverId]));
          setExpandedServerIds((current) =>
            current.includes(serverId) ? current : [...current, serverId],
          );
        }}
        onToggleServer={toggleServerExpansion}
      />

      <section className="content-shell">
        <section
          className={`content-page ${activeView.kind === "overview" ? "" : "is-hidden"}`}
        >
          {renderOverviewPage(activeView.kind === "overview")}
        </section>

        {visitedServers.map((server) => (
          <section
            key={server.id}
            className={`content-page ${activeView.kind === "server" && activeView.serverId === server.id ? "" : "is-hidden"}`}
          >
            {renderServerPage(
              server,
              activeView.kind === "server" && activeView.serverId === server.id,
            )}
          </section>
        ))}

        {visitedContainerIds.map((targetId) => {
          const target = targetById.get(targetId);
          const isVisible = activeView.kind === "container" && activeView.targetId === targetId;

          return (
            <section
              className={`content-page ${isVisible ? "" : "is-hidden"}`}
              key={targetId}
            >
              {target ? (
                renderContainerPage(target, isVisible)
              ) : (
                <section className="state-panel">
                  <span className="state-label">Unavailable</span>
                  <p className="state-copy">
                    The selected navigation target is no longer available.
                  </p>
                </section>
              )}
            </section>
          );
        })}
      </section>
    </main>
  );
}

export default App;
