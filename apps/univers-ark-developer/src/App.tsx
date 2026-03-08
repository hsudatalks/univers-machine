import { useEffect, useRef, useState, type CSSProperties } from "react";
import type { BrowserFrameInstance } from "./components/BrowserPane";
import { ContainerPage } from "./components/ContainerPage";
import { OverviewPage } from "./components/OverviewPage";
import { SettingsPage } from "./components/SettingsPage";
import { ShellState } from "./components/ShellState";
import { ServerPage } from "./components/ServerPage";
import { SidebarNav } from "./components/SidebarNav";
import { StatusBar } from "./components/StatusBar";
import { restartContainer } from "./lib/tauri";
import "./App.css";
import { useContainerWorkspace } from "./hooks/useContainerWorkspace";
import { useOverviewNavigation } from "./hooks/useOverviewNavigation";
import {
  OVERVIEW_ZOOM_DEFAULT,
  OVERVIEW_ZOOM_MAX,
  OVERVIEW_ZOOM_MIN,
  OVERVIEW_ZOOM_STEP,
  useOverviewZoom,
} from "./hooks/useOverviewZoom";
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
  type ActiveView,
} from "./lib/view-types";
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
      surfaceId: surface.id,
      state: "direct",
      message: `${surface.label} is available directly without a managed tunnel.`,
    };
  }

  return {
    targetId,
    surfaceId: surface.id,
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
  const [activeView, setActiveView] = useState<ActiveView>({ kind: "overview" });
  const [visitedContainerIds, setVisitedContainerIds] = useState<string[]>([]);
  const [visitedServerIds, setVisitedServerIds] = useState<string[]>([]);
  const previousNonSettingsViewRef = useRef<ActiveView>({ kind: "overview" });
  const { bootstrap, error, expandedServerIds, isRefreshing, refreshInventory, setExpandedServerIds } =
    useWorkbenchBootstrap();
  const { isSidebarHidden, setIsSidebarHidden } = useSidebarState();
  const { overviewZoom, setOverviewZoom, clampOverviewZoom, roundOverviewZoom } =
    useOverviewZoom();
  const { tunnelStatuses, setTunnelStatus } = useTunnelStatuses();
  const {
    activeContainerServer,
    activeContainerTarget,
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
    restartBrowserTunnel,
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

  if (error) {
    return <ShellState label="Error" message={error} />;
  }

  if (!bootstrap) {
    return <ShellState label="Loading" message="Preparing target definitions." />;
  }

  const isOverviewView = activeView.kind === "overview" || activeView.kind === "settings";
  const activeStatusLabel =
    activeView.kind === "overview"
      ? "Overview"
      : activeView.kind === "settings"
        ? "Settings"
        : activeView.kind === "server"
          ? `Server ${visitedServers.find((server) => server.id === activeView.serverId)?.label ?? activeView.serverId}`
          : `Container ${activeContainerTarget?.label ?? activeView.targetId}`;

  return (
    <main className={`shell ${isOverviewView ? "shell-overview" : ""}`}>
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
            availableTargetIds={bootstrap.targets.map((target) => target.id)}
          bootstrap={bootstrap}
          expandedServerIds={expandedServerIds}
          isOverviewActive={activeView.kind === "overview"}
          isOverviewLayout={isOverviewView}
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
        ) : null}

        <section className={`content-shell ${isOverviewView ? "content-shell-overview" : ""}`}>
        <section
          className={`content-page content-page-overview ${activeView.kind === "overview" ? "" : "is-hidden"}`}
        >
            <OverviewPage
              activeFocusedTargetId={activeOverviewFocusedTargetId}
              onFocusTarget={setOverviewFocusedTargetId}
              onOpenWorkspace={setContainerView}
              onRefreshInventory={refreshInventory}
              isRefreshing={isRefreshing}
              onResetZoom={() => {
                setOverviewZoom(OVERVIEW_ZOOM_DEFAULT);
              }}
              onZoomIn={() => {
                setOverviewZoom((current) =>
                  roundOverviewZoom(clampOverviewZoom(current + OVERVIEW_ZOOM_STEP)),
                );
              }}
              onZoomOut={() => {
                setOverviewZoom((current) =>
                  roundOverviewZoom(clampOverviewZoom(current - OVERVIEW_ZOOM_STEP)),
                );
              }}
              overviewContainers={overviewContainers}
              overviewZoom={overviewZoom}
              overviewZoomDefault={OVERVIEW_ZOOM_DEFAULT}
              overviewZoomMax={OVERVIEW_ZOOM_MAX}
              overviewZoomMin={OVERVIEW_ZOOM_MIN}
              overviewZoomStyle={{
                "--overview-terminal-grid-min-width": `${30 * overviewZoom}rem`,
                "--overview-terminal-card-height": `${32 * overviewZoom}rem`,
                "--overview-terminal-min-height": `${30 * overviewZoom}rem`,
              } as CSSProperties}
              pageVisible={activeView.kind === "overview"}
              registerOverviewCardElement={registerOverviewCardElement}
              serverCount={bootstrap.servers.length}
              standaloneTargets={standaloneTargets}
            />
          </section>

          <section
            className={`content-page content-page-overview ${activeView.kind === "settings" ? "" : "is-hidden"}`}
          >
            <SettingsPage
              configPath={bootstrap.configPath}
              onConfigSaved={refreshInventory}
              servers={bootstrap.servers}
              targets={bootstrap.targets}
              tunnelStatuses={tunnelStatuses}
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
            const activeTool = target ? (containerTools[target.id] ?? "files") : "files";
            const developmentSurface = target?.surfaces.find(
              (surface) => surface.id === "development",
            );
            const previewSurface = target?.surfaces.find(
              (surface) => surface.id === "preview",
            );
            const developmentPanel = developmentSurface
              ? (`browser:${developmentSurface.id}` as const)
              : null;
            const previewPanel = previewSurface
              ? (`browser:${previewSurface.id}` as const)
              : null;
            const activeBrowserSurfaceId = browserSurfaceIdFromPanel(activeTool);
            const browserSurface =
              activeBrowserSurfaceId && target
                ? target.surfaces.find((surface) => surface.id === activeBrowserSurfaceId)
                : undefined;
            const developmentStatus =
              developmentSurface && target
                ? tunnelStatuses[surfaceKey(target.id, developmentSurface.id)] ??
                  fallbackTunnelStatus(target.id, developmentSurface)
                : undefined;
            const previewStatus =
              previewSurface && target
                ? tunnelStatuses[surfaceKey(target.id, previewSurface.id)] ??
                  fallbackTunnelStatus(target.id, previewSurface)
                : undefined;
            const developmentBrowserFrame: BrowserFrameInstance | undefined =
              developmentSurface && developmentStatus && target
                ? {
                    cacheKey: surfaceKey(target.id, developmentSurface.id),
                    frameVersion:
                      browserFrameVersions[surfaceKey(target.id, developmentSurface.id)] ?? 0,
                    isActive: isVisible && activeTool === developmentPanel,
                    status: developmentStatus,
                    surface: developmentSurface,
                    target,
                  }
                : undefined;
            const previewBrowserFrame: BrowserFrameInstance | undefined =
              previewSurface && previewStatus && target
                ? {
                    cacheKey: surfaceKey(target.id, previewSurface.id),
                    frameVersion:
                      browserFrameVersions[surfaceKey(target.id, previewSurface.id)] ?? 0,
                    isActive: isVisible && activeTool === previewPanel,
                    status: previewStatus,
                    surface: previewSurface,
                    target,
                  }
                : undefined;

            return (
              <section
                className={`content-page ${isVisible ? "" : "is-hidden"}`}
                key={targetId}
              >
                {target ? (
                  <ContainerPage
                    activeTool={activeTool}
                    developmentPanel={developmentPanel}
                    developmentBrowserFrame={developmentBrowserFrame}
                    developmentSurface={developmentSurface}
                    isTerminalCollapsed={Boolean(containerTerminalCollapsed[target.id])}
                    onReloadBrowser={() => {
                      if (browserSurface) {
                        reloadBrowserFrame(target.id, browserSurface.id);
                      }
                    }}
                    onRestartBrowser={() => {
                      if (browserSurface) {
                        restartBrowserTunnel(target.id, browserSurface.id);
                      }
                    }}
                    onRestartContainer={
                      target.id.includes("::")
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
                    previewPanel={previewPanel}
                    previewBrowserFrame={previewBrowserFrame}
                    previewSurface={previewSurface}
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
        onToggleSidebar={() => {
          setIsSidebarHidden((current) => !current);
        }}
        overviewZoom={overviewZoom}
        reachableContainerCount={reachableContainerCount}
        serverCount={bootstrap.servers.length}
      />
    </main>
  );
}

export default App;
