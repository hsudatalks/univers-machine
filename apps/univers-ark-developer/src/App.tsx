import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { BrowserFrameInstance } from "./components/BrowserPane";
import { AddMachineDialog } from "./components/AddMachineDialog";
import { ContainerPage } from "./components/ContainerPage";
import { GlobalDashboardPage } from "./components/GlobalDashboardPage";
import { OverviewPage } from "./components/OverviewPage";
import { ServerDialog } from "./components/ServerDialog";
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
  backgroundPrerenderBrowserServices,
  browserSurfaceById,
  primaryBrowserSurface,
  resolveDefaultToolPanel,
  webServices,
} from "./lib/target-services";
import { preloadBrowserFrames } from "./lib/browser-cache";
import { registerTunnelRequests } from "./lib/tunnel-manager";
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
import { useOrchestrationViewMode } from "./hooks/useOrchestrationViewMode";
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
  hashForActiveView,
  isBrowserToolPanel,
  parseActiveViewFromHash,
  sameActiveView,
  type ActiveView,
} from "./lib/view-types";
const DEFAULT_TERMINAL_PANEL_WIDTH_REM = 35;
const MIN_TERMINAL_PANEL_WIDTH_REM = 35;
const MIN_TOOL_PANEL_WIDTH_REM = 22;
const STARTUP_PRERENDER_INITIAL_BATCH = 2;
const STARTUP_PRERENDER_BATCH_SIZE = 2;
const STARTUP_PRERENDER_BATCH_INTERVAL_MS = 1500;
type EditingMachineState = {
  initialTab: "general" | "connection" | "discovery" | "containers";
  machineId: string;
};

function resolvePreferredTarget(
  bootstrap: AppBootstrap,
  preferredTargetId?: string,
): DeveloperTarget | undefined {
  const hiddenHostTargetIds = new Set(
    bootstrap.machines.map((machine) => machine.hostTargetId),
  );
  const visibleTargets = bootstrap.targets.filter(
    (target) => !hiddenHostTargetIds.has(target.id),
  );

  if (preferredTargetId) {
    const preferredTarget = visibleTargets.find(
      (target) => target.id === preferredTargetId,
    );

    if (preferredTarget) {
      return preferredTarget;
    }
  }

  return (
    visibleTargets.find(
      (target) => target.id === bootstrap.selectedTargetId,
    ) ?? visibleTargets[0]
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
  target: DeveloperTarget,
  surface: DeveloperSurface,
): TunnelStatus {
  if (target.transport === "local" && !surface.tunnelCommand.trim()) {
    return {
      targetId: target.id,
      serviceId: surface.id,
      surfaceId: surface.id,
      localUrl: surface.localUrl,
      state: "direct",
      message: `${surface.label} is available directly without a managed tunnel.`,
    };
  }

  return {
    targetId: target.id,
    serviceId: surface.id,
    surfaceId: surface.id,
    localUrl: surface.localUrl,
    state: "starting",
    message: `${surface.label} is warming in the background.`,
  };
}

function isReadyTunnelState(state: string | undefined): boolean {
  return state === "direct" || state === "running";
}

function containerViewRefreshKey(target: DeveloperTarget): string {
  return JSON.stringify({
    label: target.label,
    description: target.description,
    notes: target.notes,
    services: target.services,
    surfaces: target.surfaces,
    terminalCommand: target.terminalCommand,
    terminalStartupCommand: target.terminalStartupCommand,
    workspace: target.workspace,
  });
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
  const [isDocumentVisible, setIsDocumentVisible] = useState(
    () =>
      typeof document === "undefined" || document.visibilityState === "visible",
  );
  const [activeView, setActiveView] = useState<ActiveView>(
    () => parseActiveViewFromHash(window.location.hash) ?? { kind: "dashboard" },
  );
  const [isAddMachineDialogOpen, setIsAddMachineDialogOpen] = useState(false);
  const [isCreatingMachine, setIsCreatingMachine] = useState(false);
  const [editingMachine, setEditingMachine] = useState<EditingMachineState | null>(null);
  const [visitedContainerIds, setVisitedContainerIds] = useState<string[]>([]);
  const [visitedMachineIds, setVisitedMachineIds] = useState<string[]>([]);
  const previousNonSettingsViewRef = useRef<ActiveView>(activeView);
  const {
    appSettings,
    resolvedTheme,
    updateDashboardRefreshSeconds,
    updateThemeMode,
  } = useAppearance();
  const {
    bootstrap,
    error,
    expandedMachineIds,
    isRefreshing,
    refreshInventory,
    setExpandedMachineIds,
  } = useWorkbenchBootstrap();
  const { isSidebarHidden, setIsSidebarHidden } = useSidebarState();
  const { overviewZoom, setOverviewZoom, clampOverviewZoom, roundOverviewZoom } =
    useOverviewZoom();
  const { orchestrationViewMode, setOrchestrationViewMode } =
    useOrchestrationViewMode();
  const { tunnelStatuses, setTunnelStatus } = useTunnelStatuses();
  const { serviceStatuses } = useServiceStatuses();
  const {
    activeContainerMachine,
    activeContainerTarget,
    overviewContainers,
    overviewTerminalTargets,
    reachableContainerCount,
    standaloneTargets,
    targetById,
    visitedMachines,
  } = useWorkbenchInventory(bootstrap, activeView, visitedMachineIds);
  const startupPrerenderDescriptors = useMemo(
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
  const editingMachineRecord = editingMachine && bootstrap
    ? bootstrap.machines.find((machine) => machine.id === editingMachine.machineId) ?? null
    : null;
  const activeStartupPrerenderDescriptors = useMemo(
    () => startupPrerenderDescriptors.slice(0, startupPrerenderBudget),
    [startupPrerenderBudget, startupPrerenderDescriptors],
  );
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
    const handleVisibilityChange = () => {
      setIsDocumentVisible(document.visibilityState === "visible");
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, []);

  useEffect(() => {
    if (startupPrerenderDescriptors.length === 0) {
      setStartupPrerenderBudget(0);
      return;
    }

    if (!isDocumentVisible) {
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
    startupPrerenderBudget,
    startupPrerenderDescriptors.length,
  ]);

  useEffect(() => {
    if (activeView.kind !== "settings") {
      previousNonSettingsViewRef.current = activeView;
    }
  }, [activeView]);

  useEffect(() => {
    const handleHashChange = () => {
      const nextView =
        parseActiveViewFromHash(window.location.hash) ?? { kind: "dashboard" as const };

      setActiveView((current) =>
        sameActiveView(current, nextView) ? current : nextView,
      );
    };

    window.addEventListener("hashchange", handleHashChange);

    return () => {
      window.removeEventListener("hashchange", handleHashChange);
    };
  }, []);

  useEffect(() => {
    const nextHash = hashForActiveView(activeView);

    if (window.location.hash === nextHash) {
      return;
    }

    if (!window.location.hash) {
      window.history.replaceState(
        null,
        "",
        `${window.location.pathname}${window.location.search}${nextHash}`,
      );
      return;
    }

    window.location.hash = nextHash;
  }, [activeView]);

  useEffect(() => {
    if (activeStartupPrerenderDescriptors.length === 0) {
      return;
    }

    void registerTunnelRequests(
      activeStartupPrerenderDescriptors.map(({ target, serviceId }) => ({
        targetId: target.id,
        serviceId,
      })),
      setTunnelStatus,
    );
  }, [activeStartupPrerenderDescriptors, setTunnelStatus]);

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
    if (!isDocumentVisible || activeStartupPrerenderDescriptors.length === 0) {
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
    startupPrerenderVersions,
    tunnelStatuses,
  ]);

  useEffect(() => {
    if (!bootstrap) {
      return;
    }

    let nextView = activeView;

    if (
      activeView.kind === "machine" &&
      !bootstrap.machines.some((machine) => machine.id === activeView.machineId)
    ) {
      nextView = { kind: "dashboard" };
    }

    if (
      activeView.kind === "container" &&
      !bootstrap.targets.some((target) => target.id === activeView.targetId)
    ) {
      nextView = { kind: "dashboard" };
    }

    if (!sameActiveView(activeView, nextView)) {
      setActiveView(nextView);
    }
  }, [activeView, bootstrap]);

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

  useEffect(() => {
    if (activeView.kind !== "machine") {
      return;
    }

    setVisitedMachineIds((current) =>
      current.includes(activeView.machineId)
        ? current
        : uniqueStrings([...current, activeView.machineId]),
    );
    setExpandedMachineIds((current) =>
      current.includes(activeView.machineId)
        ? current
        : [...current, activeView.machineId],
    );
  }, [activeView, setExpandedMachineIds]);

  useEffect(() => {
    if (activeView.kind !== "container") {
      return;
    }

    const nextTarget = targetById.get(activeView.targetId);

    if (!nextTarget) {
      return;
    }

    const isVisited = visitedContainerIds.includes(activeView.targetId);

    if (!isVisited) {
      setVisitedContainerIds((current) =>
        uniqueStrings([...current, activeView.targetId]),
      );
      prepareContainerView(nextTarget);
    }
  }, [activeView, prepareContainerView, targetById, visitedContainerIds]);

  function setContainerView(targetId: string) {
    const nextTarget = targetById.get(targetId);

    if (!nextTarget) {
      return;
    }

    setActiveView({ kind: "container", targetId });
    setVisitedContainerIds((current) => uniqueStrings([...current, targetId]));
    prepareContainerView(nextTarget);
  }

  const toggleMachineExpansion = (machineId: string) => {
    setExpandedMachineIds((current) =>
      current.includes(machineId)
        ? current.filter((entry) => entry !== machineId)
        : [...current, machineId],
    );
  };

  const openMachineView = (machineId: string) => {
    setActiveView({ kind: "machine", machineId });
    setVisitedMachineIds((current) => uniqueStrings([...current, machineId]));
    setExpandedMachineIds((current) =>
      current.includes(machineId) ? current : [...current, machineId],
    );
  };

  const openMachineSettings = (
    machineId: string,
    initialTab: EditingMachineState["initialTab"] = "general",
  ) => {
    setEditingMachine({
      machineId,
      initialTab,
    });
  };

  useEffect(() => {
    let unlistenParentView: (() => void) | undefined;

    void listenParentViewRequested(() => {
      const resolveMachineForTarget = (targetId: string) => {
        if (!bootstrap) {
          return undefined;
        }

        return bootstrap.machines.find((machine) =>
          machine.containers.some((container) => container.targetId === targetId),
        );
      };

      setActiveView((current) => {
        if (current.kind === "container") {
          const machine = resolveMachineForTarget(current.targetId);

          if (machine) {
            setVisitedMachineIds((visited) =>
              uniqueStrings([...visited, machine.id]),
            );
            setExpandedMachineIds((expanded) =>
              expanded.includes(machine.id) ? expanded : [...expanded, machine.id],
            );
            return { kind: "machine", machineId: machine.id };
          }

          return { kind: "dashboard" };
        }

        if (current.kind === "machine" || current.kind === "overview") {
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
  }, [bootstrap, setExpandedMachineIds]);

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
        ? "Orchestration"
        : activeView.kind === "settings"
          ? "Settings"
          : activeView.kind === "machine"
            ? `Machine ${visitedMachines.find((machine) => machine.id === activeView.machineId)?.label ?? activeView.machineId}`
            : `Container ${activeContainerTarget?.label ?? activeView.targetId}`;

  return (
    <main className={`shell ${isOverviewView ? "shell-overview" : ""} ${isSidebarHidden ? "shell-sidebar-hidden" : ""}`}>
      <section
        className={`shell-layout ${isOverviewView ? "shell-layout-overview" : ""} ${isSidebarHidden ? "shell-layout-sidebar-hidden" : ""}`}
      >
        {!isSidebarHidden ? (
        <SidebarNav
          activeMachineId={
            activeView.kind === "machine"
              ? activeView.machineId
              : activeView.kind === "container"
                  ? activeContainerMachine?.id
                  : undefined
            }
            activeTargetId={activeView.kind === "container" ? activeView.targetId : undefined}
            availableTargetIds={bootstrap.targets.map((target) => target.id)}
          bootstrap={bootstrap}
          expandedMachineIds={expandedMachineIds}
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
            onSelectMachine={openMachineView}
            onToggleMachine={toggleMachineExpansion}
          />
        ) : null}

        <section
          className={`content-shell ${isOverviewView ? "content-shell-overview" : ""} ${activeView.kind === "container" ? "content-shell-container" : ""}`}
        >
        <section
          className={`content-page ${activeView.kind === "dashboard" ? "" : "is-hidden"}`}
        >
          <GlobalDashboardPage
            onAddMachine={() => {
              setIsAddMachineDialogOpen(true);
            }}
            onEditAgentTeam={(machineId) => {
              openMachineSettings(machineId, "containers");
            }}
            onEditMachine={(machineId) => {
              openMachineSettings(machineId, "general");
            }}
            onOpenOverview={() => {
              setActiveView({ kind: "overview" });
            }}
            onOpenMachine={openMachineView}
            onOpenWorkspace={setContainerView}
            overviewContainers={overviewContainers}
            serviceStatuses={serviceStatuses}
            machines={bootstrap.machines}
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
              orchestrationViewMode={orchestrationViewMode}
              overviewContainers={overviewContainers}
              overviewZoom={overviewZoom}
              overviewZoomStyle={{
                "--overview-terminal-grid-min-width": `${30 * overviewZoom}rem`,
                "--overview-terminal-card-height": `${32 * overviewZoom}rem`,
                "--overview-terminal-min-height": `${30 * overviewZoom}rem`,
                "--overview-focus-side-card-height": `${16 * overviewZoom}rem`,
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
              onAddMachine={() => setIsAddMachineDialogOpen(true)}
              onDashboardRefreshChange={updateDashboardRefreshSeconds}
              onConfigSaved={refreshInventory}
              onThemeModeChange={updateThemeMode}
              resolvedTheme={resolvedTheme}
              machines={bootstrap.machines}
            />
          </section>

          {visitedMachines.map((machine) => (
            <section
              key={machine.id}
              className={`content-page ${activeView.kind === "machine" && activeView.machineId === machine.id ? "" : "is-hidden"}`}
            >
              <ServerPage
                onOpenSettings={() => {
                  openMachineSettings(machine.id, "general");
                }}
                onOpenWorkspace={setContainerView}
                pageVisible={
                  activeView.kind === "machine" && activeView.machineId === machine.id
                }
                resolveTarget={(targetId) => targetById.get(targetId)}
                server={machine}
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
                  fallbackTunnelStatus(target, browserSurface)
                : undefined;
            const browserFrames: BrowserFrameInstance[] = target
              ? webServices(target).map((service) => {
                  const surface = service.web;
                  const panel = `browser:${surface.id}` as const;
                  const status =
                    tunnelStatuses[surfaceKey(target.id, surface.id)] ??
                    fallbackTunnelStatus(target, surface);

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
                  fallbackTunnelStatus(target, primarySurface)
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
                      target.containerKind === "managed"
                        ? async () => {
                            await restartContainer(target.machineId, target.containerId);
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
                    key={`${target.id}:${containerViewRefreshKey(target)}`}
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
        isOrchestrationActive={activeView.kind === "overview"}
        isSidebarHidden={isSidebarHidden}
        onSetOrchestrationViewMode={setOrchestrationViewMode}
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
        orchestrationViewMode={orchestrationViewMode}
        overviewZoom={overviewZoom}
        overviewZoomDefault={OVERVIEW_ZOOM_DEFAULT}
        overviewZoomMax={OVERVIEW_ZOOM_MAX}
        overviewZoomMin={OVERVIEW_ZOOM_MIN}
        reachableContainerCount={reachableContainerCount}
        serverCount={bootstrap.machines.length}
      />

      {isAddMachineDialogOpen ? (
        <AddMachineDialog
          onClose={() => {
            setIsAddMachineDialogOpen(false);
          }}
          onImported={() => {
            void refreshInventory();
          }}
          onOpenCustom={() => {
            setIsCreatingMachine(true);
          }}
        />
      ) : null}

      {isCreatingMachine ? (
        <ServerDialog
          onClose={() => {
            setIsCreatingMachine(false);
          }}
          onSaved={() => {
            setIsCreatingMachine(false);
            void refreshInventory();
          }}
        />
      ) : null}

      {editingMachineRecord && editingMachine ? (
        <ServerDialog
          onClose={() => {
            setEditingMachine(null);
          }}
          onSaved={() => {
            setEditingMachine(null);
            void refreshInventory();
          }}
          initialTab={editingMachine.initialTab}
          server={editingMachineRecord}
        />
      ) : null}
    </main>
  );
}

export default App;
