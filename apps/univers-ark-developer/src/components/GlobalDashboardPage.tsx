import { Activity, Boxes, LayoutDashboard, Server, SquareTerminal } from "lucide-react";
import { primaryBrowserSurface } from "../lib/target-services";
import type {
  DeveloperTarget,
  ManagedContainer,
  ManagedMachine,
  ServiceStatus,
} from "../types";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

interface OverviewEntry {
  container: ManagedContainer;
  machine: ManagedMachine;
  target?: DeveloperTarget;
}

interface GlobalDashboardPageProps {
  onOpenOverview: () => void;
  onOpenMachine: (machineId: string) => void;
  onOpenWorkspace: (targetId: string) => void;
  overviewContainers: OverviewEntry[];
  serviceStatuses: Record<string, ServiceStatus>;
  machines: ManagedMachine[];
  standaloneTargets: DeveloperTarget[];
}

function inventoryStateVariant(
  state: string,
): "neutral" | "success" | "warning" | "destructive" {
  switch (state) {
    case "ready":
      return "success";
    case "degraded":
    case "empty":
      return "warning";
    case "error":
      return "destructive";
    default:
      return "neutral";
  }
}

function sshStateVariant(
  reachable: boolean,
): "neutral" | "success" | "warning" | "destructive" {
  return reachable ? "success" : "destructive";
}

function serviceKey(targetId: string, serviceId: string): string {
  return `${targetId}::${serviceId}`;
}

export function GlobalDashboardPage({
  onOpenOverview,
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
  const activeWebServiceCount = overviewContainers.filter((entry) => {
    if (!entry.target) {
      return false;
    }

    const primary = primaryBrowserSurface(entry.target);

    if (!primary) {
      return false;
    }

    const state = serviceStatuses[serviceKey(entry.target.id, primary.id)]?.state;

    return (
      state === "running" ||
      state === "ready" ||
      state === "healthy" ||
      state === "loaded" ||
      state === "direct"
    );
  }).length;

  return (
    <section className="page-section">
      <article className="panel tool-panel dashboard-panel global-dashboard-panel">
        <div className="dashboard-grid">
          <Card className="dashboard-card-hero border-border/80 bg-card/95">
            <CardContent className="dashboard-summary-bar">
              <div className="dashboard-summary-copy">
                <div className="dashboard-summary-item">
                  <span className="dashboard-meta-label">Machines</span>
                  <span className="dashboard-meta-value">{machines.length}</span>
                </div>
                <div className="dashboard-summary-item">
                  <span className="dashboard-meta-label">Agent teams</span>
                  <span className="dashboard-meta-value">{overviewContainers.length}</span>
                </div>
                <div className="dashboard-summary-item">
                  <span className="dashboard-meta-label">SSH ready</span>
                  <span className="dashboard-meta-value">
                    {reachableContainerCount} / {overviewContainers.length}
                  </span>
                </div>
                <div className="dashboard-summary-item">
                  <span className="dashboard-meta-label">Web ready</span>
                  <span className="dashboard-meta-value">{activeWebServiceCount}</span>
                </div>
              </div>

              <div className="dashboard-summary-actions">
                <Button onClick={onOpenOverview} size="sm" variant="ghost">
                  <LayoutDashboard size={14} />
                  Open overview
                </Button>
              </div>
            </CardContent>
          </Card>

          <Card className="border-border/80 bg-card/95">
            <CardHeader>
              <CardTitle className="dashboard-section-title">
                <Server size={16} />
                Machines
              </CardTitle>
            </CardHeader>
            <CardContent className="server-dashboard-list">
              {machines.map((machine) => {
                const reachable = machine.containers.filter((item) => item.sshReachable).length;

                return (
                  <div className="server-dashboard-row" key={machine.id}>
                    <div className="server-dashboard-row-copy">
                      <span className="server-dashboard-row-title">{machine.label}</span>
                      <span className="server-dashboard-row-meta">
                        {machine.host} · {machine.containers.length} container(s) · {reachable} ssh ready
                      </span>
                    </div>

                    <div className="server-dashboard-row-actions">
                      <Badge variant={inventoryStateVariant(machine.state)}>
                        {machine.state}
                      </Badge>
                      <Button
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

          <Card className="dashboard-card-wide border-border/80 bg-card/95">
            <CardHeader>
              <CardTitle className="dashboard-section-title">
                <Boxes size={16} />
                Agent teams
              </CardTitle>
            </CardHeader>
            <CardContent className="server-dashboard-list">
              {overviewContainers.map(({ container, machine, target }) => {
                const primary = target ? primaryBrowserSurface(target) : undefined;
                const webState = primary && target
                  ? serviceStatuses[serviceKey(target.id, primary.id)]?.state
                  : undefined;

                return (
                  <div className="server-dashboard-row" key={container.targetId}>
                    <div className="server-dashboard-row-copy">
                      <span className="server-dashboard-row-title">{container.label}</span>
                      <span className="server-dashboard-row-meta">
                        {machine.label} · {container.ipv4 || "no ip"} · web {webState ?? "unknown"}
                      </span>
                    </div>

                    <div className="server-dashboard-row-actions">
                      <Badge variant={sshStateVariant(container.sshReachable)}>
                        {container.sshState}
                      </Badge>
                      {primary ? (
                        <Badge variant="neutral">{primary.label}</Badge>
                      ) : null}
                      {target ? (
                        <Button
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
                <div className="server-dashboard-row" key={target.id}>
                  <div className="server-dashboard-row-copy">
                    <span className="server-dashboard-row-title">{target.label}</span>
                    <span className="server-dashboard-row-meta">
                      Standalone target
                    </span>
                  </div>

                  <div className="server-dashboard-row-actions">
                    <Button
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

          <Card className="border-border/80 bg-card/95">
            <CardHeader>
              <CardTitle className="dashboard-section-title">
                <Activity size={16} />
                Governance
              </CardTitle>
            </CardHeader>
            <CardContent>
              <ul className="dashboard-actions-list">
                <li className="dashboard-action-item">
                  <span className="dashboard-action-title">Dashboard</span>
                  <p className="dashboard-copy">
                    Software-wide operational view across machines and agent teams.
                  </p>
                </li>
                <li className="dashboard-action-item">
                  <span className="dashboard-action-title">Overview</span>
                  <p className="dashboard-copy">
                    High-density terminal wall for direct supervision and quick drill-down.
                  </p>
                </li>
              </ul>
            </CardContent>
          </Card>
        </div>
      </article>
    </section>
  );
}
