import type { CSSProperties } from "react";
import { GlobalDashboardPage } from "../../components/GlobalDashboardPage";
import { HomeMachinesPage } from "../../components/HomeMachinesPage";
import { OverviewPage } from "../../components/OverviewPage";
import type { HomeViewMode } from "../../hooks/useOrchestrationViewMode";
import type {
  DeveloperTarget,
  ManagedContainer,
  ManagedMachine,
  ServiceStatus,
} from "../../types";

interface OverviewEntry {
  container: ManagedContainer;
  machine: ManagedMachine;
  target?: DeveloperTarget;
}

interface HomeContentProps {
  activeMachineOverviewFocusedTargetId: string;
  activeTerminalOverviewFocusedTargetId: string;
  homeViewMode: HomeViewMode;
  isCompactHomeLayout: boolean;
  isRefreshing: boolean;
  machines: ManagedMachine[];
  onAddProvider: () => void;
  onEditProvider: (machineId: string) => void;
  onEditWorkbench: (machineId: string) => void;
  onFocusMachineTarget: (targetId: string) => void;
  onFocusTerminalTarget: (targetId: string) => void;
  onOpenProvider: (machineId: string) => void;
  onOpenWorkbench: (targetId: string) => void;
  onOpenGrid: () => void;
  onOpenProviders: () => void;
  onRefreshInventory: () => void;
  overviewContainers: OverviewEntry[];
  overviewZoom: number;
  pageVisible: boolean;
  registerMachineOverviewCardElement: (targetId: string, element: HTMLElement | null) => void;
  registerTerminalOverviewCardElement: (targetId: string, element: HTMLElement | null) => void;
  resolveTarget: (targetId: string) => DeveloperTarget | undefined;
  serviceStatuses: Record<string, ServiceStatus>;
  standaloneTargets: DeveloperTarget[];
}

function overviewZoomStyle(overviewZoom: number): CSSProperties {
  return {
    "--overview-terminal-grid-min-width": `${30 * overviewZoom}rem`,
    "--overview-terminal-card-height": `${32 * overviewZoom}rem`,
    "--overview-terminal-min-height": `${30 * overviewZoom}rem`,
    "--overview-focus-side-card-height": `${16 * overviewZoom}rem`,
  } as CSSProperties;
}

export function HomeContent({
  activeMachineOverviewFocusedTargetId,
  activeTerminalOverviewFocusedTargetId,
  homeViewMode,
  isCompactHomeLayout,
  isRefreshing,
  machines,
  onAddProvider,
  onEditProvider,
  onEditWorkbench,
  onFocusMachineTarget,
  onFocusTerminalTarget,
  onOpenProvider,
  onOpenWorkbench,
  onOpenGrid,
  onOpenProviders,
  onRefreshInventory,
  overviewContainers,
  overviewZoom,
  pageVisible,
  registerMachineOverviewCardElement,
  registerTerminalOverviewCardElement,
  resolveTarget,
  serviceStatuses,
  standaloneTargets,
}: HomeContentProps) {
  if (homeViewMode === "dashboard") {
    return (
      <GlobalDashboardPage
        machines={machines}
        onAddMachine={onAddProvider}
        onEditMachine={onEditProvider}
        onEditWorkbench={onEditWorkbench}
        onOpenGrid={isCompactHomeLayout ? undefined : onOpenGrid}
        onOpenMachine={onOpenProvider}
        onOpenMachines={isCompactHomeLayout ? undefined : onOpenProviders}
        onOpenWorkspace={onOpenWorkbench}
        overviewContainers={overviewContainers}
        serviceStatuses={serviceStatuses}
        standaloneTargets={standaloneTargets}
      />
    );
  }

  if (homeViewMode === "machines") {
    return (
      <HomeMachinesPage
        activeFocusedTargetId={activeMachineOverviewFocusedTargetId}
        machines={machines}
        onAddMachine={onAddProvider}
        onFocusTarget={onFocusMachineTarget}
        onOpenMachine={onOpenProvider}
        overviewZoom={overviewZoom}
        overviewZoomStyle={overviewZoomStyle(overviewZoom)}
        pageVisible={pageVisible}
        registerOverviewCardElement={registerMachineOverviewCardElement}
        resolveTarget={resolveTarget}
      />
    );
  }

  return (
    <OverviewPage
      activeFocusedTargetId={activeTerminalOverviewFocusedTargetId}
      homeViewMode={homeViewMode}
      isRefreshing={isRefreshing}
      onFocusTarget={onFocusTerminalTarget}
      onOpenWorkspace={onOpenWorkbench}
      onRefreshInventory={onRefreshInventory}
      overviewContainers={overviewContainers}
      overviewZoom={overviewZoom}
      overviewZoomStyle={overviewZoomStyle(overviewZoom)}
      pageVisible={pageVisible}
      registerOverviewCardElement={registerTerminalOverviewCardElement}
      standaloneTargets={standaloneTargets}
    />
  );
}
