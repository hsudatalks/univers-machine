import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { BrowserFrameInstance } from "./components/BrowserPane";
import { AddMachineDialog } from "./components/AddMachineDialog";
import { ContainerPage } from "./components/ContainerPage";
import { GlobalDashboardPage } from "./components/GlobalDashboardPage";
import { HomeMachinesPage } from "./components/HomeMachinesPage";
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
  updateRuntimeActivity,
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
const IS_MAC = navigator.platform.toUpperCase().includes("MAC");
const HOME_VIEW_MODES = ["dashboard", "machines", "grid", "focus"] as const;

type EditingMachineState = {
  initialTab: "general" | "connection" | "discovery" | "containers";
  machineId: string;
};

function isPlatformModifier(event: KeyboardEvent): boolean {
  return IS_MAC ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

function isEditableEventTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    target.isContentEditable
  );
}

function isXtermHelperTextarea(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLTextAreaElement &&
    target.classList.contains("xterm-helper-textarea")
  );
}

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
  const [isWindowFocused, setIsWindowFocused] = useState(
    () => typeof document === "undefined" || document.hasFocus(),
  );
  const [isNetworkOnline, setIsNetworkOnline] = useState(
    () => typeof navigator === "undefined" || navigator.onLine,
  );
  const [isMobileSidebarOpen, setIsMobileSidebarOpen] = useState(false);
  const [activeView, setActiveView] = useState<ActiveView>(
    () => parseActiveViewFromHash(window.location.hash) ?? { kind: "home" },
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
    if (activeView.kind !== "settings") {
      previousNonSettingsViewRef.current = activeView;
    }
  }, [activeView]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        !isPlatformModifier(event) ||
        event.altKey ||
        event.shiftKey ||
        event.code !== "KeyS" ||
        isEditableEventTarget(event.target)
      ) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      setActiveView((current) =>
        current.kind === "settings"
          ? previousNonSettingsViewRef.current
          : { kind: "settings" },
      );
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        activeView.kind !== "home" ||
        !isPlatformModifier(event) ||
        event.altKey ||
        (isCompactHomeLayout
          ? event.code !== "Digit1"
          : event.key !== "Tab" &&
            event.code !== "Digit1" &&
            event.code !== "Digit2" &&
            event.code !== "Digit3" &&
            event.code !== "Digit4") ||
        (isEditableEventTarget(event.target) && !isXtermHelperTextarea(event.target))
      ) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      if (event.code === "Digit1" && !event.shiftKey) {
        setHomeViewMode("dashboard");
        return;
      }

      if (isCompactHomeLayout) {
        return;
      }

      if (event.code === "Digit2" && !event.shiftKey) {
        setHomeViewMode("machines");
        return;
      }

      if (event.code === "Digit3" && !event.shiftKey) {
        setHomeViewMode("grid");
        return;
      }

      if (event.code === "Digit4" && !event.shiftKey) {
        setHomeViewMode("focus");
        return;
      }

      if (event.key !== "Tab") {
        return;
      }

      const currentIndex = HOME_VIEW_MODES.indexOf(homeViewMode);
      const nextIndex =
        currentIndex === -1
          ? 0
          : (currentIndex +
              (event.shiftKey ? HOME_VIEW_MODES.length - 1 : 1)) %
            HOME_VIEW_MODES.length;

      setHomeViewMode(HOME_VIEW_MODES[nextIndex]);
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [activeView.kind, homeViewMode, isCompactHomeLayout, setHomeViewMode]);

  useEffect(() => {
    const machines = bootstrap?.machines ?? [];

    if (activeView.kind !== "machine" || machines.length <= 1) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        !isPlatformModifier(event) ||
        !event.altKey ||
        event.shiftKey ||
        (event.key !== "ArrowLeft" && event.key !== "ArrowRight") ||
        (isEditableEventTarget(event.target) && !isXtermHelperTextarea(event.target))
      ) {
        return;
      }

      const currentIndex =
        machines.findIndex((machine) => machine.id === activeView.machineId);

      if (currentIndex === -1) {
        return;
      }

      const nextIndex =
        event.key === "ArrowLeft"
          ? (currentIndex + machines.length - 1) % machines.length
          : (currentIndex + 1) % machines.length;
      const nextMachine = machines[nextIndex];

      event.preventDefault();
      event.stopPropagation();
      openMachineView(nextMachine.id);
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [activeView, bootstrap]);

  useEffect(() => {
    const handleHashChange = () => {
      const nextView =
        parseActiveViewFromHash(window.location.hash) ?? { kind: "home" as const };

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
    onSetOverviewFocus: setTerminalOverviewFocusedTargetId,
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
    activeView.kind === "home" || activeView.kind === "settings";
  const isSidebarVisible = !isCompactWorkspaceLayout && !isSidebarHidden;
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
    <main className={`shell ${isHomeLayout ? "shell-overview" : ""} ${isSidebarVisible ? "" : "shell-sidebar-hidden"}`}>
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
            {homeViewMode === "dashboard" ? (
              <GlobalDashboardPage
                onAddMachine={() => {
                  setIsAddMachineDialogOpen(true);
                }}
                onEditWorkbench={(machineId) => {
                  openMachineSettings(machineId, "containers");
                }}
                onEditMachine={(machineId) => {
                  openMachineSettings(machineId, "general");
                }}
                onOpenGrid={
                  isCompactHomeLayout
                    ? undefined
                    : () => {
                      setHomeViewMode("grid");
                    }
                }
                onOpenMachines={
                  isCompactHomeLayout
                    ? undefined
                    : () => {
                      setHomeViewMode("machines");
                    }
                }
                onOpenMachine={openMachineView}
                onOpenWorkspace={setContainerView}
                overviewContainers={overviewContainers}
                serviceStatuses={serviceStatuses}
                machines={bootstrap.machines}
                standaloneTargets={standaloneTargets}
              />
            ) : homeViewMode === "machines" ? (
              <HomeMachinesPage
                activeFocusedTargetId={activeMachineOverviewFocusedTargetId}
                machines={bootstrap.machines}
                onAddMachine={() => {
                  setIsAddMachineDialogOpen(true);
                }}
                onFocusTarget={setMachineOverviewFocusedTargetId}
                onOpenMachine={openMachineView}
                overviewZoom={overviewZoom}
                overviewZoomStyle={{
                  "--overview-terminal-grid-min-width": `${30 * overviewZoom}rem`,
                  "--overview-terminal-card-height": `${32 * overviewZoom}rem`,
                  "--overview-terminal-min-height": `${30 * overviewZoom}rem`,
                  "--overview-focus-side-card-height": `${16 * overviewZoom}rem`,
                } as CSSProperties}
                pageVisible={activeView.kind === "home"}
                registerOverviewCardElement={registerMachineOverviewCardElement}
                resolveTarget={(targetId) => targetById.get(targetId)}
              />
            ) : (
              <OverviewPage
                activeFocusedTargetId={activeTerminalOverviewFocusedTargetId}
                onFocusTarget={setTerminalOverviewFocusedTargetId}
                onOpenWorkspace={setContainerView}
                onRefreshInventory={refreshInventory}
                isRefreshing={isRefreshing}
                homeViewMode={homeViewMode}
                overviewContainers={overviewContainers}
                overviewZoom={overviewZoom}
                overviewZoomStyle={{
                  "--overview-terminal-grid-min-width": `${30 * overviewZoom}rem`,
                  "--overview-terminal-card-height": `${32 * overviewZoom}rem`,
                  "--overview-terminal-min-height": `${30 * overviewZoom}rem`,
                  "--overview-focus-side-card-height": `${16 * overviewZoom}rem`,
                } as CSSProperties}
                pageVisible={activeView.kind === "home"}
                registerOverviewCardElement={registerTerminalOverviewCardElement}
                standaloneTargets={standaloneTargets}
              />
            )}
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

      {isCompactWorkspaceLayout ? (
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

      <StatusBar
        activeMachineId={activeView.kind === "machine" ? activeView.machineId : undefined}
        activeStatusLabel={activeStatusLabel}
        containerCount={overviewContainers.length}
        homeViewModes={isCompactHomeLayout ? ["dashboard"] : HOME_VIEW_MODES}
        isHomeActive={activeView.kind === "home"}
        isSidebarHidden={isCompactWorkspaceLayout ? !isMobileSidebarOpen : isSidebarHidden}
        machineEntries={bootstrap.machines.map((machine) => ({ id: machine.id, label: machine.label }))}
        onNavigateMachine={openMachineView}
        onSetHomeViewMode={setHomeViewMode}
        onOpenSettings={() => {
          setActiveView((current) =>
            current.kind === "settings"
              ? previousNonSettingsViewRef.current
              : { kind: "settings" },
          );
        }}
        onToggleSidebar={handleStatusBarSidebarToggle}
        homeViewMode={homeViewMode}
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
          onSaved={(event) => {
            if (event?.close !== false) {
              setIsCreatingMachine(false);
            }
            void refreshInventory();
          }}
        />
      ) : null}

      {editingMachineRecord && editingMachine ? (
        <ServerDialog
          onClose={() => {
            setEditingMachine(null);
          }}
          onSaved={(event) => {
            if (event?.close !== false) {
              setEditingMachine(null);
            }
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
