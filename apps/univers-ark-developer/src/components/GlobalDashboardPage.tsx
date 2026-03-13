import { Boxes, LayoutDashboard, Server, Settings2, SquareTerminal } from "lucide-react";
import { useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { visibleContainers } from "../lib/container-visibility";
import { primaryBrowserSurface } from "../lib/target-services";
import type {
  DeveloperTarget,
  ManagedContainer,
  ManagedMachine,
  ServiceStatus,
} from "../types";
import { ConnectionStatusLight } from "./ConnectionStatusLight";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

interface OverviewEntry {
  container: ManagedContainer;
  machine: ManagedMachine;
  target?: DeveloperTarget;
}

interface GlobalDashboardPageProps {
  onAddMachine: () => void;
  onEditWorkbench: (machineId: string) => void;
  onEditMachine: (machineId: string) => void;
  onOpenGrid?: () => void;
  onOpenMachines?: () => void;
  onOpenMachine: (machineId: string) => void;
  onOpenWorkspace: (targetId: string) => void;
  overviewContainers: OverviewEntry[];
  serviceStatuses: Record<string, ServiceStatus>;
  machines: ManagedMachine[];
  standaloneTargets: DeveloperTarget[];
}

function serviceKey(targetId: string, serviceId: string): string {
  return `${targetId}::${serviceId}`;
}

function isSshReadyState(state: string | undefined): boolean {
  switch ((state || "").trim().toLowerCase()) {
    case "ready":
    case "running":
    case "connected":
    case "direct":
      return true;
    default:
      return false;
  }
}

function hostAvailabilityLabel(state: string | undefined): string {
  switch ((state || "").trim().toLowerCase()) {
    case "ready":
    case "running":
    case "connected":
    case "direct":
      return "host ready";
    case "checking":
    case "starting":
    case "pending":
      return "host checking";
    default:
      return "host unavailable";
  }
}

const MOBILE_DASHBOARD_BREAKPOINT = 720;
const PROVIDER_PANEL_MAX_RATIO = 0.36;

export function GlobalDashboardPage({
  onAddMachine,
  onEditWorkbench,
  onEditMachine,
  onOpenGrid,
  onOpenMachines,
  onOpenMachine,
  onOpenWorkspace,
  overviewContainers,
  serviceStatuses,
  machines,
  standaloneTargets,
}: GlobalDashboardPageProps) {
  const reachableContainerCount = overviewContainers.filter(
    (entry) => entry.container.sshReachable,
  ).length;
  const reachableMachineCount = machines.filter((machine) =>
    isSshReadyState(machine.state),
  ).length;
  const totalSshEndpointCount = machines.length + overviewContainers.length;
  const reachableSshEndpointCount = reachableMachineCount + reachableContainerCount;
  const gridRef = useRef<HTMLDivElement>(null);
  const summaryCardRef = useRef<HTMLDivElement>(null);
  const providersHeaderRef = useRef<HTMLDivElement>(null);
  const providersContentRef = useRef<HTMLDivElement>(null);
  const workbenchesHeaderRef = useRef<HTMLDivElement>(null);
  const workbenchesContentRef = useRef<HTMLDivElement>(null);
  const [dashboardGridRows, setDashboardGridRows] = useState<string | undefined>();

  useLayoutEffect(() => {
    const updateLayout = () => {
      const grid = gridRef.current;
      const summaryCard = summaryCardRef.current;
      const providersHeader = providersHeaderRef.current;
      const providersContent = providersContentRef.current;
      const workbenchesHeader = workbenchesHeaderRef.current;
      const workbenchesContent = workbenchesContentRef.current;

      if (
        !grid ||
        !summaryCard ||
        !providersHeader ||
        !providersContent ||
        !workbenchesHeader ||
        !workbenchesContent
      ) {
        return;
      }

      if (window.innerWidth <= MOBILE_DASHBOARD_BREAKPOINT) {
        setDashboardGridRows(undefined);
        return;
      }

      const computedStyle = window.getComputedStyle(grid);
      const rowGap = Number.parseFloat(computedStyle.rowGap || computedStyle.gap || "0") || 0;
      const availableHeight = Math.max(
        0,
        grid.clientHeight - summaryCard.offsetHeight - rowGap * 2,
      );

      if (availableHeight <= 0) {
        setDashboardGridRows(undefined);
        return;
      }

      const providersNaturalHeight =
        providersHeader.offsetHeight + providersContent.scrollHeight;
      const workbenchesNaturalHeight =
        workbenchesHeader.offsetHeight + workbenchesContent.scrollHeight;
      const providerCap = Math.max(0, availableHeight * PROVIDER_PANEL_MAX_RATIO);

      let providersHeight = 0;
      let workbenchesHeight = 0;

      if (providersNaturalHeight + workbenchesNaturalHeight <= availableHeight) {
        providersHeight = providersNaturalHeight;
        workbenchesHeight = workbenchesNaturalHeight;
      } else {
        providersHeight = Math.min(providersNaturalHeight, providerCap);
        workbenchesHeight = Math.min(
          workbenchesNaturalHeight,
          Math.max(0, availableHeight - providersHeight),
        );
      }

      setDashboardGridRows(
        `auto minmax(0, ${Math.round(providersHeight)}px) minmax(0, ${Math.round(
          workbenchesHeight,
        )}px)`,
      );
    };

    let frame = 0;
    const scheduleUpdate = () => {
      cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(updateLayout);
    };

    scheduleUpdate();

    const resizeObserver = new ResizeObserver(() => {
      scheduleUpdate();
    });

    if (gridRef.current) {
      resizeObserver.observe(gridRef.current);
    }

    window.addEventListener("resize", scheduleUpdate);

    return () => {
      cancelAnimationFrame(frame);
      resizeObserver.disconnect();
      window.removeEventListener("resize", scheduleUpdate);
    };
  }, [machines, overviewContainers, standaloneTargets]);

  const dashboardGridStyle = dashboardGridRows
    ? ({ gridTemplateRows: dashboardGridRows } satisfies CSSProperties)
    : undefined;

  return (
    <section className="page-section global-dashboard-page">
      <article className="panel tool-panel dashboard-panel global-dashboard-panel">
        <div className="dashboard-grid" ref={gridRef} style={dashboardGridStyle}>
          <Card
            className="dashboard-card dashboard-card-hero border-border/80 bg-card/95"
            ref={summaryCardRef}
          >
            <CardContent className="dashboard-summary-bar dashboard-card-content">
              <div className="dashboard-summary-copy">
                <div className="dashboard-summary-item">
                  <span className="dashboard-meta-label">Providers</span>
                  <span className="dashboard-meta-value">{machines.length}</span>
                </div>
                <div className="dashboard-summary-item">
                  <span className="dashboard-meta-label">Workbenches</span>
                  <span className="dashboard-meta-value">{overviewContainers.length}</span>
                </div>
                <div className="dashboard-summary-item">
                  <span className="dashboard-meta-label">SSH ready</span>
                  <span className="dashboard-meta-value">
                    {reachableSshEndpointCount} / {totalSshEndpointCount}
                  </span>
                </div>
              </div>

              <div className="dashboard-summary-actions">
                {onOpenMachines ? (
                  <Button
                    className="dashboard-summary-button"
                    onClick={onOpenMachines}
                    size="sm"
                    variant="ghost"
                  >
                    <Server size={14} />
                    Open providers
                  </Button>
                ) : null}
                {onOpenGrid ? (
                  <Button
                    className="dashboard-summary-button"
                    onClick={onOpenGrid}
                    size="sm"
                    variant="ghost"
                  >
                    <LayoutDashboard size={14} />
                    Open grid
                  </Button>
                ) : null}
              </div>
            </CardContent>
          </Card>

          <Card className="dashboard-card dashboard-card-wide dashboard-card-scroll-shell border-border/80 bg-card/95">
            <CardHeader className="dashboard-card-header" ref={providersHeaderRef}>
              <div className="dashboard-card-header-row">
                <CardTitle className="dashboard-section-title">
                  <Server size={16} />
                  Providers
                </CardTitle>
                <Button className="dashboard-summary-button" onClick={onAddMachine} size="sm">
                  <Server size={14} />
                  Add provider
                </Button>
              </div>
            </CardHeader>
            <CardContent
              className="server-dashboard-list dashboard-card-list dashboard-card-content dashboard-card-scroll"
              ref={providersContentRef}
            >
              {machines.map((machine) => {
                const managedContainers = visibleContainers(machine.containers);
                const reachable = managedContainers.filter((item) => item.sshReachable).length;
                const machineMeta = managedContainers.length
                  ? `${machine.host} · ${managedContainers.length} container(s) · ${reachable} ssh ready`
                  : `${machine.host} · ${hostAvailabilityLabel(machine.state)}`;

                return (
                  <div className="server-dashboard-row dashboard-card-row" key={machine.id}>
                    <div className="server-dashboard-row-copy">
                      <span className="server-dashboard-row-title">{machine.label}</span>
                      <span className="server-dashboard-row-meta">
                        {machineMeta}
                      </span>
                    </div>

                    <div className="server-dashboard-row-actions">
                      <ConnectionStatusLight className="dashboard-row-status" state={machine.state} />
                      <Badge className="dashboard-row-badge" variant="neutral">
                        Machine provider
                      </Badge>
                      <Button
                        aria-label={`Edit ${machine.label}`}
                        className="dashboard-row-icon-button"
                        onClick={() => {
                          onEditMachine(machine.id);
                        }}
                        size="icon"
                        title="Machine settings"
                        variant="ghost"
                      >
                        <Settings2 size={14} />
                      </Button>
                      <Button
                        className="dashboard-row-open-button"
                        onClick={() => {
                          onOpenMachine(machine.id);
                        }}
                        size="sm"
                        variant="ghost"
                      >
                        <Server size={14} />
                        Open
                      </Button>
                    </div>
                  </div>
                );
              })}
            </CardContent>
          </Card>

          <Card className="dashboard-card dashboard-card-wide dashboard-card-scroll-shell border-border/80 bg-card/95">
            <CardHeader className="dashboard-card-header" ref={workbenchesHeaderRef}>
              <CardTitle className="dashboard-section-title">
                <Boxes size={16} />
                Workbenches
              </CardTitle>
            </CardHeader>
            <CardContent
              className="server-dashboard-list dashboard-card-list dashboard-card-content dashboard-card-scroll"
              ref={workbenchesContentRef}
            >
              {overviewContainers.map(({ container, machine, target }) => {
                const primary = target ? primaryBrowserSurface(target) : undefined;
                const webState = primary && target
                  ? serviceStatuses[serviceKey(target.id, primary.id)]?.state
                  : undefined;

                return (
                  <div className="server-dashboard-row dashboard-card-row" key={container.targetId}>
                    <div className="server-dashboard-row-copy">
                      <span className="server-dashboard-row-title">{container.label}</span>
                      <span className="server-dashboard-row-meta">
                        {machine.label} · {container.ipv4 || "no ip"} · web {webState ?? "unknown"}
                      </span>
                    </div>

                    <div className="server-dashboard-row-actions">
                      <ConnectionStatusLight
                        className="dashboard-row-status"
                        state={container.sshState}
                        title={container.sshState}
                      />
                      {primary ? (
                        <Badge className="dashboard-row-badge" variant="neutral">
                          {primary.label}
                        </Badge>
                      ) : null}
                        <Button
                          aria-label={`Edit ${container.label}`}
                          className="dashboard-row-icon-button"
                          onClick={() => {
                            onEditWorkbench(machine.id);
                          }}
                          size="icon"
                          title="Container settings"
                        variant="ghost"
                      >
                        <Settings2 size={14} />
                      </Button>
                      {target ? (
                        <Button
                          className="dashboard-row-open-button"
                          onClick={() => {
                            onOpenWorkspace(target.id);
                          }}
                          size="sm"
                          variant="ghost"
                        >
                          <SquareTerminal size={14} />
                          Open
                        </Button>
                      ) : null}
                    </div>
                  </div>
                );
              })}

              {standaloneTargets.length > 0 ? (
                <div className="dashboard-global-section-label">
                  Standalone
                </div>
              ) : null}

              {standaloneTargets.map((target) => (
                <div className="server-dashboard-row dashboard-card-row" key={target.id}>
                  <div className="server-dashboard-row-copy">
                    <span className="server-dashboard-row-title">{target.label}</span>
                    <span className="server-dashboard-row-meta">
                      Standalone target
                    </span>
                  </div>

                  <div className="server-dashboard-row-actions">
                    <Button
                      className="dashboard-row-open-button"
                      onClick={() => {
                        onOpenWorkspace(target.id);
                      }}
                      size="sm"
                      variant="ghost"
                    >
                      <SquareTerminal size={14} />
                      Open
                    </Button>
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
        </div>
      </article>
    </section>
  );
}
