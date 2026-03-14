import { useEffect, useMemo, useState } from "react";
import { GlobalDialogs, type EditingMachineState } from "./app/GlobalDialogs";
import { useAppNavigation } from "./app/useAppNavigation";
import { useGlobalShortcuts } from "./app/useGlobalShortcuts";
import { useStartupBrowserPrerender } from "./app/useStartupBrowserPrerender";
import { SettingsPage } from "./components/SettingsPage";
import { ShellState } from "./components/ShellState";
import { SidebarNav } from "./components/SidebarNav";
import { StatusBar } from "./components/StatusBar";
import { HomeContent } from "./features/home/HomeContent";
import { ProviderPages } from "./features/provider/ProviderPages";
import { WorkbenchPages } from "./features/workbench/WorkbenchPages";
import {
  closeContainerCompanionWindow,
  executeCommandService,
  listenCompanionTargetRequested,
  listenParentViewRequested,
  restartContainer,
  syncContainerCompanionWindow,
  updateRuntimeActivity,
} from "./lib/tauri";
import "./App.css";
import { useAppearance } from "./hooks/useAppearance";
import { useContainerWorkspace } from "./hooks/useContainerWorkspace";
import { useMediaQuery } from "./hooks/useMediaQuery";
import { useOverviewNavigation } from "./hooks/useOverviewNavigation";
import {
  useOverviewZoom,
} from "./hooks/useOverviewZoom";
import { useHomeViewMode } from "./hooks/useOrchestrationViewMode";
import { useServiceStatuses } from "./hooks/useServiceStatuses";
import { useSidebarState } from "./hooks/useSidebarState";
import { useTunnelStatuses } from "./hooks/useTunnelStatuses";
import { useWorkbenchBootstrap } from "./hooks/useWorkbenchBootstrap";
import { useWorkbenchInventory } from "./hooks/useWorkbenchInventory";
import type {
  AppBootstrap,
  DeveloperTarget,
} from "./types";
import { sameActiveView } from "./lib/view-types";

const DEFAULT_TERMINAL_PANEL_WIDTH_REM = 35;
const MIN_TERMINAL_PANEL_WIDTH_REM = 35;
const MIN_TOOL_PANEL_WIDTH_REM = 22;
const HOME_VIEW_MODES = ["dashboard", "machines", "grid", "focus"] as const;

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

function rootFontSizePx(): number {
  if (typeof window === "undefined") {
    return 16;
  }

  const parsed = Number.parseFloat(
    window.getComputedStyle(document.documentElement).fontSize,
  );

  return Number.isFinite(parsed) ? parsed : 16;
}

function isCompanionWindowMode(): boolean {
  if (typeof window === "undefined") {
    return false;
  }

  return new URLSearchParams(window.location.search).get("mode") === "companion";
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
  const isCompanionWindow = useMemo(() => isCompanionWindowMode(), []);
  const { activeView, setActiveView, toggleSettingsView } = useAppNavigation();
  const [isDocumentVisible, setIsDocumentVisible] = useState(
    () =>
      typeof document === "undefined" || document.visibilityState === "visible",
  );
  const [isWindowFocused, setIsWindowFocused] = useState(
    () => typeof document === "undefined" || document.hasFocus(),
  );
  const [isNetworkOnline, setIsNetworkOnline] = useState(
    () => typeof navigator === "undefined" || navigator.onLine,
  );
  const [isMobileSidebarOpen, setIsMobileSidebarOpen] = useState(false);
  const [isAddMachineDialogOpen, setIsAddMachineDialogOpen] = useState(false);
  const [isCreatingMachine, setIsCreatingMachine] = useState(false);
  const [editingMachine, setEditingMachine] = useState<EditingMachineState | null>(null);
  const [visitedContainerIds, setVisitedContainerIds] = useState<string[]>([]);
  const [visitedMachineIds, setVisitedMachineIds] = useState<string[]>([]);
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
  const { overviewZoom } = useOverviewZoom();
  const { homeViewMode, setHomeViewMode } = useHomeViewMode();
  const isCompactHomeLayout = useMediaQuery("(max-width: 960px)");
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
  const editingMachineRecord = editingMachine && bootstrap
    ? bootstrap.machines.find((machine) => machine.id === editingMachine.machineId) ?? null
    : null;
  const homeMachineTargetIds = useMemo(
    () =>
      bootstrap?.machines
        .map((machine) => machine.hostTargetId)
        .filter((targetId) => Boolean(targetById.get(targetId))) ?? [],
    [bootstrap, targetById],
  );
  const homeTerminalTargetIds = useMemo(
    () => overviewTerminalTargets.map((target) => target.id),
    [overviewTerminalTargets],
  );
  const {
    activeOverviewFocusedTargetId: activeMachineOverviewFocusedTargetId,
    registerOverviewCardElement: registerMachineOverviewCardElement,
    setOverviewFocusedTargetId: setMachineOverviewFocusedTargetId,
  } = useOverviewNavigation({
    isNavigationActive: activeView.kind === "home" && homeViewMode === "machines",
    onOpenWorkspace: (targetId) => {
      const machine = bootstrap?.machines.find(
        (item) => item.hostTargetId === targetId,
      );

      if (machine) {
        openMachineView(machine.id);
      }
    },
    targetIds: homeMachineTargetIds,
  });
  const {
    activeOverviewFocusedTargetId: activeTerminalOverviewFocusedTargetId,
    registerOverviewCardElement: registerTerminalOverviewCardElement,
    setOverviewFocusedTargetId: setTerminalOverviewFocusedTargetId,
  } = useOverviewNavigation({
    isNavigationActive:
      activeView.kind === "home" &&
      (homeViewMode === "grid" || homeViewMode === "focus"),
    onOpenWorkspace: setContainerView,
    targetIds: homeTerminalTargetIds,
  });
  const activeHomeFocusedTargetId =
    homeViewMode === "machines"
      ? activeMachineOverviewFocusedTargetId
      : activeTerminalOverviewFocusedTargetId;
  const activeFocusCompanionTarget =
    activeView.kind === "home" && homeViewMode === "focus" && activeTerminalOverviewFocusedTargetId
      ? targetById.get(activeTerminalOverviewFocusedTargetId) ?? null
      : null;
  const isCompactWorkspaceLayout = isCompactHomeLayout
    && activeView.kind !== "home"
    && activeView.kind !== "settings";

  useEffect(() => {
    if (!isCompactWorkspaceLayout && isMobileSidebarOpen) {
      setIsMobileSidebarOpen(false);
    }
  }, [isCompactWorkspaceLayout, isMobileSidebarOpen]);

  useEffect(() => {
    if (!isCompactWorkspaceLayout) {
      return;
    }

    setIsMobileSidebarOpen(false);
  }, [activeView, isCompactWorkspaceLayout]);
  const activeRuntimeTargetId = useMemo(() => {
    if (activeView.kind === "container") {
      return activeView.targetId;
    }

    if (activeView.kind === "home" && homeViewMode !== "dashboard") {
      return activeHomeFocusedTargetId || null;
    }

    return null;
  }, [activeHomeFocusedTargetId, activeView, homeViewMode]);
  const activeRuntimeMachineId = useMemo(() => {
    if (activeView.kind === "machine") {
      return activeView.machineId;
    }

    if (activeView.kind === "container") {
      return activeContainerMachine?.id ?? activeContainerTarget?.machineId ?? null;
    }

    if (activeView.kind === "home" && homeViewMode !== "dashboard" && activeRuntimeTargetId) {
      return targetById.get(activeRuntimeTargetId)?.machineId ?? null;
    }

    return null;
  }, [
    activeContainerMachine?.id,
    activeContainerTarget?.machineId,
    activeRuntimeTargetId,
    activeView,
    homeViewMode,
    targetById,
  ]);

  useEffect(() => {
    if (!isCompactHomeLayout || homeViewMode === "dashboard") {
      return;
    }

    setHomeViewMode("dashboard");
  }, [homeViewMode, isCompactHomeLayout, setHomeViewMode]);

  useEffect(() => {
    if (!bootstrap) {
      return;
    }

    const initialTarget = resolvePreferredTarget(bootstrap);
    const initialMachineTargetId = bootstrap.machines
      .map((machine) => machine.hostTargetId)
      .find((targetId) => Boolean(targetById.get(targetId)));

    setTerminalOverviewFocusedTargetId(
      (current) => current || initialTarget?.id || "",
    );
    setMachineOverviewFocusedTargetId(
      (current) => current || initialMachineTargetId || "",
    );
  }, [
    bootstrap,
    setMachineOverviewFocusedTargetId,
    setTerminalOverviewFocusedTargetId,
    targetById,
  ]);

  useEffect(() => {
    const handleVisibilityChange = () => {
      setIsDocumentVisible(document.visibilityState === "visible");
    };
    const handleFocus = () => {
      setIsWindowFocused(true);
    };
    const handleBlur = () => {
      setIsWindowFocused(false);
    };
    const handleOnline = () => {
      setIsNetworkOnline(true);
    };
    const handleOffline = () => {
      setIsNetworkOnline(false);
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    window.addEventListener("focus", handleFocus);
    window.addEventListener("blur", handleBlur);
    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);

    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      window.removeEventListener("focus", handleFocus);
      window.removeEventListener("blur", handleBlur);
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("offline", handleOffline);
    };
  }, []);

  useEffect(() => {
    void updateRuntimeActivity({
      visible: isDocumentVisible,
      focused: isWindowFocused,
      online: isNetworkOnline,
      activeMachineId: activeRuntimeMachineId,
      activeTargetId: activeRuntimeTargetId,
    }).catch(() => undefined);
  }, [
    activeRuntimeMachineId,
    activeRuntimeTargetId,
    isDocumentVisible,
    isNetworkOnline,
    isWindowFocused,
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
      nextView = { kind: "home" };
    }

    if (
      activeView.kind === "container" &&
      !bootstrap.targets.some((target) => target.id === activeView.targetId)
    ) {
      nextView = { kind: "home" };
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
    resetBrowserFrame,
    selectContainerTool,
    setContainerTerminalCollapsedState,
    startContainerResize,
    toggleTerminalCollapsed,
  } = useContainerWorkspace({
    activeView,
    clampTerminalPanelWidth,
    defaultTerminalPanelWidthPx,
    onSetActiveView: setActiveView,
    onSelectContainer: setContainerView,
    onSetOverviewFocus: setTerminalOverviewFocusedTargetId,
    onTunnelStatus: setTunnelStatus,
    orderedTargetIds: overviewTerminalTargets.map((target) => target.id),
    serviceStatuses,
    tunnelStatuses,
    targetById,
  });

  useEffect(() => {
    if (!isCompanionWindow) {
      return;
    }

    let unlistenCompanionTarget: (() => void) | undefined;

    void listenCompanionTargetRequested(({ targetId }) => {
      const nextTarget = targetById.get(targetId);

      if (!nextTarget) {
        return;
      }

      setActiveView({ kind: "container", targetId });
      setVisitedContainerIds((current) =>
        uniqueStrings([...current, targetId]),
      );
      prepareContainerView(nextTarget);
      setContainerTerminalCollapsedState(targetId, true);
    }).then((dispose) => {
      unlistenCompanionTarget = dispose;
    });

    return () => {
      unlistenCompanionTarget?.();
    };
  }, [
    isCompanionWindow,
    prepareContainerView,
    setContainerTerminalCollapsedState,
    targetById,
  ]);

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

  useEffect(() => {
    if (!isCompanionWindow || activeView.kind !== "container") {
      return;
    }

    setContainerTerminalCollapsedState(activeView.targetId, true);
  }, [activeView, isCompanionWindow, setContainerTerminalCollapsedState]);

  useEffect(() => {
    if (isCompanionWindow) {
      return;
    }

    if (!activeFocusCompanionTarget) {
      void closeContainerCompanionWindow().catch(() => undefined);
      return;
    }

    void syncContainerCompanionWindow(
      activeFocusCompanionTarget.id,
      activeFocusCompanionTarget.label,
    ).catch(() => undefined);
  }, [activeFocusCompanionTarget, isCompanionWindow]);

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

  useGlobalShortcuts({
    activeFocusCompanionTarget,
    activeView,
    homeViewMode,
    isCompanionWindow,
    isCompactHomeLayout,
    machines: bootstrap?.machines ?? [],
    onOpenMachine: openMachineView,
    onSetHomeViewMode: setHomeViewMode,
    onToggleSettings: toggleSettingsView,
  });

  useStartupBrowserPrerender({
    bootstrap,
    isDocumentVisible,
    isNetworkOnline,
    onTunnelStatus: setTunnelStatus,
    tunnelStatuses,
  });

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

          return { kind: "home" };
        }

        if (current.kind === "machine" || current.kind === "home") {
          return { kind: "home" };
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

  const isHomeLayout =
    !isCompanionWindow &&
    (activeView.kind === "home" || activeView.kind === "settings");
  const isSidebarVisible =
    !isCompanionWindow && !isCompactWorkspaceLayout && !isSidebarHidden;
  const activeStatusLabel =
    activeView.kind === "home"
      ? "Home"
      : activeView.kind === "settings"
        ? "Settings"
        : activeView.kind === "machine"
        ? `Machine ${visitedMachines.find((machine) => machine.id === activeView.machineId)?.label ?? activeView.machineId}`
        : `Container ${activeContainerTarget?.label ?? activeView.targetId}`;

  const closeMobileSidebar = () => {
    setIsMobileSidebarOpen(false);
  };

  const handleSidebarSelectHome = () => {
    closeMobileSidebar();
    setActiveView({ kind: "home" });
  };

  const handleSidebarSelectContainer = (targetId: string) => {
    closeMobileSidebar();
    setContainerView(targetId);
  };

  const handleSidebarSelectMachine = (machineId: string) => {
    closeMobileSidebar();
    openMachineView(machineId);
  };

  const handleStatusBarSidebarToggle = () => {
    if (isCompactWorkspaceLayout) {
      setIsMobileSidebarOpen((current) => !current);
      return;
    }

    setIsSidebarHidden((current) => !current);
  };

  return (
    <main className={`shell ${isHomeLayout ? "shell-overview" : ""} ${isSidebarVisible ? "" : "shell-sidebar-hidden"} ${isCompanionWindow ? "shell-companion" : ""}`}>
      <section
        className={`shell-layout ${isHomeLayout ? "shell-layout-overview" : ""} ${isSidebarVisible ? "" : "shell-layout-sidebar-hidden"}`}
      >
        {isSidebarVisible ? (
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
            isHomeActive={activeView.kind === "home"}
            isHomeLayout={isHomeLayout}
            onSelectContainer={handleSidebarSelectContainer}
            onSelectHome={handleSidebarSelectHome}
            onSelectMachine={handleSidebarSelectMachine}
            onToggleMachine={toggleMachineExpansion}
          />
        ) : null}

        <section
          className={`content-shell ${isHomeLayout ? "content-shell-overview" : ""} ${activeView.kind === "container" ? "content-shell-container" : ""}`}
        >
          <section
            className={`content-page content-page-overview ${activeView.kind === "home" ? "" : "is-hidden"}`}
          >
            <HomeContent
              activeMachineOverviewFocusedTargetId={activeMachineOverviewFocusedTargetId}
              activeTerminalOverviewFocusedTargetId={activeTerminalOverviewFocusedTargetId}
              homeViewMode={homeViewMode}
              isCompactHomeLayout={isCompactHomeLayout}
              isRefreshing={isRefreshing}
              machines={bootstrap.machines}
              onAddProvider={() => {
                setIsAddMachineDialogOpen(true);
              }}
              onEditProvider={(machineId) => {
                openMachineSettings(machineId, "general");
              }}
              onEditWorkbench={(machineId) => {
                openMachineSettings(machineId, "containers");
              }}
              onFocusMachineTarget={setMachineOverviewFocusedTargetId}
              onFocusTerminalTarget={setTerminalOverviewFocusedTargetId}
              onOpenGrid={() => {
                setHomeViewMode("grid");
              }}
              onOpenProvider={openMachineView}
              onOpenProviders={() => {
                setHomeViewMode("machines");
              }}
              onOpenWorkbench={setContainerView}
              onRefreshInventory={refreshInventory}
              overviewContainers={overviewContainers}
              overviewZoom={overviewZoom}
              pageVisible={activeView.kind === "home"}
              registerMachineOverviewCardElement={registerMachineOverviewCardElement}
              registerTerminalOverviewCardElement={registerTerminalOverviewCardElement}
              resolveTarget={(targetId) => targetById.get(targetId)}
              serviceStatuses={serviceStatuses}
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

          <ProviderPages
            activeMachineId={activeView.kind === "machine" ? activeView.machineId : null}
            onOpenProviderSettings={(machineId) => {
              openMachineSettings(machineId, "general");
            }}
            onOpenWorkbench={setContainerView}
            pageVisible={activeView.kind === "machine"}
            resolveTarget={(targetId) => targetById.get(targetId)}
            visitedMachines={visitedMachines}
          />

          <WorkbenchPages
            activeTargetId={activeView.kind === "container" ? activeView.targetId : null}
            browserFrameVersions={browserFrameVersions}
            containerTerminalCollapsed={containerTerminalCollapsed}
            containerTerminalWidths={containerTerminalWidths}
            containerTools={containerTools}
            dashboardRefreshSeconds={appSettings.dashboardRefreshSeconds}
            defaultTerminalPanelWidthPx={defaultTerminalPanelWidthPx}
            onExecuteCommandService={executeCommandService}
            onResetBrowser={resetBrowserFrame}
            onRestartContainer={restartContainer}
            onSelectTool={selectContainerTool}
            onStartResize={startContainerResize}
            onToggleTerminalCollapsed={toggleTerminalCollapsed}
            serviceStatuses={serviceStatuses}
            targetById={targetById}
            tunnelStatuses={tunnelStatuses}
            visitedTargetIds={visitedContainerIds}
          />
        </section>
      </section>

      {isCompactWorkspaceLayout && !isCompanionWindow ? (
        <div className={`mobile-sidebar-overlay ${isMobileSidebarOpen ? "is-open" : ""}`}>
          <button
            aria-label="Close workspace navigation"
            className="mobile-sidebar-backdrop"
            onClick={closeMobileSidebar}
            type="button"
          />

          <div className="mobile-sidebar-drawer" role="dialog" aria-label="Workspace navigation">
            <SidebarNav
              activeMachineId={activeView.kind === "machine" ? activeView.machineId : activeContainerMachine?.id}
              activeTargetId={activeView.kind === "container" ? activeView.targetId : undefined}
              availableTargetIds={bootstrap.targets.map((target) => target.id)}
              bootstrap={bootstrap}
              expandedMachineIds={expandedMachineIds}
              isHomeActive={false}
              isHomeLayout={false}
              onSelectContainer={handleSidebarSelectContainer}
              onSelectHome={handleSidebarSelectHome}
              onSelectMachine={handleSidebarSelectMachine}
              onToggleMachine={toggleMachineExpansion}
            />
          </div>
        </div>
      ) : null}

      {!isCompanionWindow ? (
        <StatusBar
          activeMachineId={activeView.kind === "machine" ? activeView.machineId : undefined}
          activeTargetId={activeView.kind === "container" ? activeView.targetId : undefined}
          activeStatusLabel={activeStatusLabel}
          containerCount={overviewContainers.length}
          homeViewModes={isCompactHomeLayout ? ["dashboard"] : HOME_VIEW_MODES}
          isHomeActive={activeView.kind === "home"}
          isSidebarHidden={isCompactWorkspaceLayout ? !isMobileSidebarOpen : isSidebarHidden}
          machineEntries={bootstrap.machines.map((machine) => ({ id: machine.id, label: machine.label }))}
          onNavigateMachine={openMachineView}
          onNavigateTarget={setContainerView}
          onSetHomeViewMode={setHomeViewMode}
          onOpenSettings={toggleSettingsView}
          onToggleSidebar={handleStatusBarSidebarToggle}
          homeViewMode={homeViewMode}
          reachableContainerCount={reachableContainerCount}
          serverCount={bootstrap.machines.length}
          targetEntries={overviewTerminalTargets.map((target) => ({
            id: target.id,
            label: target.label,
          }))}
        />
      ) : null}

      <GlobalDialogs
        editingMachine={editingMachine}
        editingMachineRecord={editingMachineRecord}
        isAddMachineDialogOpen={isAddMachineDialogOpen}
        isCreatingMachine={isCreatingMachine}
        onCloseAddMachineDialog={() => {
          setIsAddMachineDialogOpen(false);
        }}
        onCloseCreatingMachine={() => {
          setIsCreatingMachine(false);
        }}
        onCloseEditingMachine={() => {
          setEditingMachine(null);
        }}
        onImportedMachine={() => {
          void refreshInventory();
        }}
        onOpenCustomMachine={() => {
          setIsCreatingMachine(true);
        }}
        onSavedCreatedMachine={(event) => {
          if (event?.close !== false) {
            setIsCreatingMachine(false);
          }
          void refreshInventory();
        }}
        onSavedEditedMachine={(event) => {
          if (event?.close !== false) {
            setEditingMachine(null);
          }
          void refreshInventory();
        }}
      />
    </main>
  );
}

export default App;
