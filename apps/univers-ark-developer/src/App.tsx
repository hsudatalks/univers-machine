import { useEffect, useRef, useState, type CSSProperties } from "react";
import type { BrowserFrameInstance } from "./components/BrowserPane";
import { ContainerPage } from "./components/ContainerPage";
import { GlobalDashboardPage } from "./components/GlobalDashboardPage";
import { OverviewPage } from "./components/OverviewPage";
import { SettingsPage } from "./components/SettingsPage";
import { ShellState } from "./components/ShellState";
import { ServerPage } from "./components/ServerPage";
import { SidebarNav } from "./components/SidebarNav";
import { StatusBar } from "./components/StatusBar";
import {
  executeCommandService,
  listenParentViewRequested,
  restartContainer,
} from "./lib/tauri";
import {
  browserSurfaceById,
  primaryBrowserSurface,
  resolveDefaultToolPanel,
  webServices,
} from "./lib/target-services";
import "./App.css";
import { useAppearance } from "./hooks/useAppearance";
import { useContainerWorkspace } from "./hooks/useContainerWorkspace";
import { useOverviewNavigation } from "./hooks/useOverviewNavigation";
import {
  OVERVIEW_ZOOM_DEFAULT,
  OVERVIEW_ZOOM_MAX,
  OVERVIEW_ZOOM_MIN,
  OVERVIEW_ZOOM_STEP,
  useOverviewZoom,
} from "./hooks/useOverviewZoom";
import { useServiceStatuses } from "./hooks/useServiceStatuses";
import { useSidebarState } from "./hooks/useSidebarState";
import { useTunnelStatuses } from "./hooks/useTunnelStatuses";
import { useWorkbenchBootstrap } from "./hooks/useWorkbenchBootstrap";
import { useWorkbenchInventory } from "./hooks/useWorkbenchInventory";
import type {
  AppBootstrap,
  DeveloperSurface,
  DeveloperTarget,
  TunnelStatus,
} from "./types";
import {
  browserSurfaceIdFromPanel,
  isBrowserToolPanel,
  type ActiveView,
} from "./lib/view-types";
import { isServerHostTargetId } from "./lib/server-targets";
const DEFAULT_TERMINAL_PANEL_WIDTH_REM = 35;
const MIN_TERMINAL_PANEL_WIDTH_REM = 35;
const MIN_TOOL_PANEL_WIDTH_REM = 22;

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

function surfaceKey(targetId: string, surfaceId: string): string {
  return `${targetId}::${surfaceId}`;
}

function fallbackTunnelStatus(
  targetId: string,
  surface: DeveloperSurface,
): TunnelStatus {
  if (!surface.tunnelCommand.trim()) {
    return {
      targetId,
      serviceId: surface.id,
      surfaceId: surface.id,
      localUrl: surface.localUrl,
      state: "direct",
      message: `${surface.label} is available directly without a managed tunnel.`,
    };
  }

  return {
    targetId,
    serviceId: surface.id,
    surfaceId: surface.id,
    localUrl: surface.localUrl,
    state: "starting",
    message: `${surface.label} is warming in the background.`,
  };
}

function rootFontSizePx(): number {
  if (typeof window === "undefined") {
    return 16;
  }

  const parsed = Number.parseFloat(
    window.getComputedStyle(document.documentElement).fontSize,
  );

  return Number.isFinite(parsed) ? parsed : 16;
}

function defaultTerminalPanelWidthPx(): number {
  return DEFAULT_TERMINAL_PANEL_WIDTH_REM * rootFontSizePx();
}

function minTerminalPanelWidthPx(): number {
  return MIN_TERMINAL_PANEL_WIDTH_REM * rootFontSizePx();
}

function minToolPanelWidthPx(): number {
  return MIN_TOOL_PANEL_WIDTH_REM * rootFontSizePx();
}

function clampTerminalPanelWidth(value: number, workspaceWidth: number): number {
  const minTerminalWidth = minTerminalPanelWidthPx();
  const maxTerminalWidth = Math.max(
    minTerminalWidth,
    workspaceWidth - minToolPanelWidthPx(),
  );

  return Math.min(maxTerminalWidth, Math.max(minTerminalWidth, value));
}

function App() {
  const [activeView, setActiveView] = useState<ActiveView>({ kind: "dashboard" });
  const [visitedContainerIds, setVisitedContainerIds] = useState<string[]>([]);
  const [visitedServerIds, setVisitedServerIds] = useState<string[]>([]);
  const previousNonSettingsViewRef = useRef<ActiveView>({ kind: "dashboard" });
  const {
    appSettings,
    resolvedTheme,
    updateDashboardRefreshSeconds,
    updateThemeMode,
  } = useAppearance();
  const { bootstrap, error, expandedServerIds, isRefreshing, refreshInventory, setExpandedServerIds } =
    useWorkbenchBootstrap();
  const { isSidebarHidden, setIsSidebarHidden } = useSidebarState();
  const { overviewZoom, setOverviewZoom, clampOverviewZoom, roundOverviewZoom } =
    useOverviewZoom();
  const { tunnelStatuses, setTunnelStatus } = useTunnelStatuses();
  const { serviceStatuses } = useServiceStatuses();
  const {
    activeContainerServer,
    activeContainerTarget,
    hostTargets,
    overviewContainers,
    overviewTerminalTargets,
    reachableContainerCount,
    standaloneTargets,
    targetById,
    visitedServers,
  } = useWorkbenchInventory(bootstrap, activeView, visitedServerIds);
  const {
    activeOverviewFocusedTargetId,
    registerOverviewCardElement,
    setOverviewFocusedTargetId,
  } = useOverviewNavigation({
    activeViewKind: activeView.kind,
    onOpenWorkspace: setContainerView,
    targetIds: overviewTerminalTargets.map((target) => target.id),
  });

  useEffect(() => {
    if (!bootstrap) {
      return;
    }

    const initialTarget = resolvePreferredTarget(bootstrap);
    setOverviewFocusedTargetId((current) => current || initialTarget?.id || "");
  }, [bootstrap, setOverviewFocusedTargetId]);

  useEffect(() => {
    if (activeView.kind !== "settings") {
      previousNonSettingsViewRef.current = activeView;
    }
  }, [activeView]);

  const {
    browserFrameVersions,
    containerTerminalCollapsed,
    containerTerminalWidths,
    containerTools,
    prepareContainerView,
    reloadBrowserFrame,
    selectContainerTool,
    startContainerResize,
    toggleTerminalCollapsed,
  } = useContainerWorkspace({
    activeView,
    clampTerminalPanelWidth,
    defaultTerminalPanelWidthPx,
    onSetActiveView: setActiveView,
    onSelectContainer: setContainerView,
    onSetOverviewFocus: setOverviewFocusedTargetId,
    onTunnelStatus: setTunnelStatus,
    orderedTargetIds: overviewTerminalTargets.map((target) => target.id),
    serviceStatuses,
    tunnelStatuses,
    targetById,
  });

  function setContainerView(targetId: string) {
    const nextTarget = targetById.get(targetId);

    if (!nextTarget) {
      return;
    }

    setActiveView({ kind: "container", targetId });
    setVisitedContainerIds((current) => uniqueStrings([...current, targetId]));
    prepareContainerView(nextTarget);
  }

  const toggleServerExpansion = (serverId: string) => {
    setExpandedServerIds((current) =>
      current.includes(serverId)
        ? current.filter((entry) => entry !== serverId)
        : [...current, serverId],
    );
  };

  const openServerView = (serverId: string) => {
    setActiveView({ kind: "server", serverId });
    setVisitedServerIds((current) => uniqueStrings([...current, serverId]));
    setExpandedServerIds((current) =>
      current.includes(serverId) ? current : [...current, serverId],
    );
  };

  useEffect(() => {
    let unlistenParentView: (() => void) | undefined;

    void listenParentViewRequested(() => {
      const resolveServerForTarget = (targetId: string) => {
        if (!bootstrap) {
          return undefined;
        }

        return bootstrap.servers.find((server) => {
          const hostTarget = hostTargets.find((target) => target.id === targetId);
          if (hostTarget) {
            return server.id === hostTarget.id.replace(/^server-host::/, "");
          }

          return server.containers.some(
            (container) => container.targetId === targetId,
          );
        });
      };

      setActiveView((current) => {
        if (current.kind === "container") {
          const server = resolveServerForTarget(current.targetId);

          if (server) {
            setVisitedServerIds((visited) =>
              uniqueStrings([...visited, server.id]),
            );
            setExpandedServerIds((expanded) =>
              expanded.includes(server.id) ? expanded : [...expanded, server.id],
            );
            return { kind: "server", serverId: server.id };
          }

          return { kind: "dashboard" };
        }

        if (current.kind === "server" || current.kind === "overview") {
          return { kind: "dashboard" };
        }

        return current;
      });
    }).then((dispose) => {
      unlistenParentView = dispose;
    });

    return () => {
      unlistenParentView?.();
    };
  }, [bootstrap, hostTargets, setExpandedServerIds]);

  if (error) {
    return <ShellState label="Error" message={error} />;
  }

  if (!bootstrap) {
    return <ShellState label="Loading" message="Preparing target definitions." />;
  }

  const isOverviewView =
    activeView.kind === "overview" || activeView.kind === "settings";
  const activeStatusLabel =
    activeView.kind === "dashboard"
      ? "Dashboard"
      : activeView.kind === "overview"
        ? "Overview"
        : activeView.kind === "settings"
          ? "Settings"
          : activeView.kind === "server"
            ? `Server ${visitedServers.find((server) => server.id === activeView.serverId)?.label ?? activeView.serverId}`
            : `Container ${activeContainerTarget?.label ?? activeView.targetId}`;

  return (
    <main className={`shell ${isOverviewView ? "shell-overview" : ""} ${isSidebarHidden ? "shell-sidebar-hidden" : ""}`}>
      <section
        className={`shell-layout ${isOverviewView ? "shell-layout-overview" : ""} ${isSidebarHidden ? "shell-layout-sidebar-hidden" : ""}`}
      >
        {!isSidebarHidden ? (
        <SidebarNav
          activeServerId={
            activeView.kind === "server"
              ? activeView.serverId
              : activeView.kind === "container"
                  ? activeContainerServer?.id
                  : undefined
            }
            activeTargetId={activeView.kind === "container" ? activeView.targetId : undefined}
            availableTargetIds={[...bootstrap.targets, ...hostTargets].map((target) => target.id)}
          bootstrap={bootstrap}
          expandedServerIds={expandedServerIds}
          isDashboardActive={activeView.kind === "dashboard"}
          isOverviewActive={activeView.kind === "overview"}
          isOverviewLayout={isOverviewView}
          onSelectContainer={setContainerView}
          onSelectDashboard={() => {
            setActiveView({ kind: "dashboard" });
          }}
          onSelectOverview={() => {
            setActiveView({ kind: "overview" });
            }}
            onSelectServer={openServerView}
            onToggleServer={toggleServerExpansion}
          />
        ) : null}

        <section
          className={`content-shell ${isOverviewView ? "content-shell-overview" : ""} ${activeView.kind === "container" ? "content-shell-container" : ""}`}
        >
        <section
          className={`content-page ${activeView.kind === "dashboard" ? "" : "is-hidden"}`}
        >
          <GlobalDashboardPage
            onOpenOverview={() => {
              setActiveView({ kind: "overview" });
            }}
            onOpenServer={openServerView}
            onOpenWorkspace={setContainerView}
            overviewContainers={overviewContainers}
            serviceStatuses={serviceStatuses}
            servers={bootstrap.servers}
            standaloneTargets={standaloneTargets}
          />
        </section>

        <section
          className={`content-page content-page-overview ${activeView.kind === "overview" ? "" : "is-hidden"}`}
        >
            <OverviewPage
              activeFocusedTargetId={activeOverviewFocusedTargetId}
              onFocusTarget={setOverviewFocusedTargetId}
              onOpenWorkspace={setContainerView}
              onRefreshInventory={refreshInventory}
              isRefreshing={isRefreshing}
              overviewContainers={overviewContainers}
              overviewZoom={overviewZoom}
              overviewZoomStyle={{
                "--overview-terminal-grid-min-width": `${30 * overviewZoom}rem`,
                "--overview-terminal-card-height": `${32 * overviewZoom}rem`,
                "--overview-terminal-min-height": `${30 * overviewZoom}rem`,
              } as CSSProperties}
              pageVisible={activeView.kind === "overview"}
              registerOverviewCardElement={registerOverviewCardElement}
              standaloneTargets={standaloneTargets}
            />
          </section>

          <section
            className={`content-page content-page-overview ${activeView.kind === "settings" ? "" : "is-hidden"}`}
          >
            <SettingsPage
              appSettings={appSettings}
              configPath={bootstrap.configPath}
              onDashboardRefreshChange={updateDashboardRefreshSeconds}
              onConfigSaved={refreshInventory}
              onThemeModeChange={updateThemeMode}
              resolvedTheme={resolvedTheme}
              servers={bootstrap.servers}
            />
          </section>

          {visitedServers.map((server) => (
            <section
              key={server.id}
              className={`content-page ${activeView.kind === "server" && activeView.serverId === server.id ? "" : "is-hidden"}`}
            >
              <ServerPage
                onOpenWorkspace={setContainerView}
                pageVisible={
                  activeView.kind === "server" && activeView.serverId === server.id
                }
                resolveTarget={(targetId) => targetById.get(targetId)}
                server={server}
              />
            </section>
          ))}

          {visitedContainerIds.map((targetId) => {
            const target = targetById.get(targetId);
            const isVisible = activeView.kind === "container" && activeView.targetId === targetId;
            const activeTool = target
              ? (containerTools[target.id] ?? resolveDefaultToolPanel(target))
              : "dashboard";
            const primarySurface = target
              ? primaryBrowserSurface(target)
              : undefined;
            const activeBrowserSurfaceId = browserSurfaceIdFromPanel(activeTool);
            const browserSurface =
              activeBrowserSurfaceId && target
                ? browserSurfaceById(target, activeBrowserSurfaceId)
                : undefined;
            const browserPanel = browserSurface
              ? (`browser:${browserSurface.id}` as const)
              : primarySurface
                ? (`browser:${primarySurface.id}` as const)
                : isBrowserToolPanel(activeTool)
                  ? activeTool
                  : null;
            const browserStatus =
              browserSurface && target
                ? tunnelStatuses[surfaceKey(target.id, browserSurface.id)] ??
                  fallbackTunnelStatus(target.id, browserSurface)
                : undefined;
            const browserFrames: BrowserFrameInstance[] = target
              ? webServices(target).map((service) => {
                  const surface = service.web;
                  const panel = `browser:${surface.id}` as const;
                  const status =
                    tunnelStatuses[surfaceKey(target.id, surface.id)] ??
                    fallbackTunnelStatus(target.id, surface);

                  return {
                    cacheKey: surfaceKey(target.id, surface.id),
                    frameVersion:
                      browserFrameVersions[surfaceKey(target.id, surface.id)] ?? 0,
                    isActive: isVisible && activeTool === panel,
                    status,
                    surface,
                    target,
                  };
                })
              : [];
            const browserFrame: BrowserFrameInstance | undefined =
              browserSurface && browserStatus && target
                ? browserFrames.find(
                    (frame) => frame.surface.id === browserSurface.id,
                  )
                : undefined;
            const primaryBrowserStatus =
              primarySurface && target
                ? tunnelStatuses[surfaceKey(target.id, primarySurface.id)] ??
                  fallbackTunnelStatus(target.id, primarySurface)
                : undefined;
            return (
              <section
                className={`content-page content-page-container ${isVisible ? "" : "is-hidden"}`}
                key={targetId}
              >
                {target ? (
                  <ContainerPage
                    activeTool={activeTool}
                    browserFrame={browserFrame}
                    browserFrames={browserFrames}
                    browserPanel={browserPanel}
                    browserServices={
                      target
                        ? webServices(target).map((service) => ({
                            id: service.id,
                            label: service.label,
                          }))
                        : []
                    }
                    browserSurface={browserSurface}
                    dashboardRefreshSeconds={appSettings.dashboardRefreshSeconds}
                    primaryBrowserStatus={primaryBrowserStatus}
                    primaryBrowserSurface={primarySurface}
                    isTerminalCollapsed={Boolean(containerTerminalCollapsed[target.id])}
                    onExecuteCommandService={(serviceId, action) =>
                      executeCommandService(target.id, serviceId, action)
                    }
                    onOpenBrowserService={(serviceId) => {
                      selectContainerTool(target, `browser:${serviceId}`);
                    }}
                    onReloadBrowser={() => {
                      if (browserSurface) {
                        reloadBrowserFrame(target.id, browserSurface.id);
                      }
                    }}
                    onRestartContainer={
                      target.id.includes("::") && !isServerHostTargetId(target.id)
                        ? async () => {
                            const [serverId, containerName] = target.id.split("::", 2);
                            await restartContainer(serverId, containerName);
                          }
                        : undefined
                    }
                    onSelectTool={(panel) => {
                      selectContainerTool(target, panel);
                    }}
                    onStartResize={(event) => {
                      startContainerResize(event, target.id);
                    }}
                    onToggleTerminalCollapsed={() => {
                      toggleTerminalCollapsed(target.id);
                    }}
                    pageVisible={isVisible}
                    serviceStatuses={serviceStatuses}
                    target={target}
                    workspaceStyle={{
                      "--container-terminal-width": `${containerTerminalWidths[target.id] ?? defaultTerminalPanelWidthPx()}px`,
                    } as CSSProperties}
                  />
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
      </section>

      <StatusBar
        activeStatusLabel={activeStatusLabel}
        containerCount={overviewContainers.length}
        isOverviewView={isOverviewView}
        isSidebarHidden={isSidebarHidden}
        onOpenSettings={() => {
          setActiveView((current) =>
            current.kind === "settings"
              ? previousNonSettingsViewRef.current
              : { kind: "settings" },
          );
        }}
        onResetOverviewZoom={() => {
          setOverviewZoom(OVERVIEW_ZOOM_DEFAULT);
        }}
        onToggleSidebar={() => {
          setIsSidebarHidden((current) => !current);
        }}
        onZoomInOverview={() => {
          setOverviewZoom((current) =>
            roundOverviewZoom(clampOverviewZoom(current + OVERVIEW_ZOOM_STEP)),
          );
        }}
        onZoomOutOverview={() => {
          setOverviewZoom((current) =>
            roundOverviewZoom(clampOverviewZoom(current - OVERVIEW_ZOOM_STEP)),
          );
        }}
        overviewZoom={overviewZoom}
        overviewZoomDefault={OVERVIEW_ZOOM_DEFAULT}
        overviewZoomMax={OVERVIEW_ZOOM_MAX}
        overviewZoomMin={OVERVIEW_ZOOM_MIN}
        reachableContainerCount={reachableContainerCount}
        serverCount={bootstrap.servers.length}
      />
    </main>
  );
}

export default App;
