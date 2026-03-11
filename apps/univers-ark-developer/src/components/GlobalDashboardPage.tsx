import { Boxes, LayoutDashboard, Server, Settings2, SquareTerminal } from "lucide-react";
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
  onEditAgentTeam: (machineId: string) => void;
  onEditMachine: (machineId: string) => void;
  onOpenOverview: () => void;
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

export function GlobalDashboardPage({
  onAddMachine,
  onEditAgentTeam,
  onEditMachine,
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
              </div>

              <div className="dashboard-summary-actions">
                <Button onClick={onAddMachine} size="sm">
                  <Server size={14} />
                  Add machine
                </Button>
                <Button onClick={onOpenOverview} size="sm" variant="ghost">
                  <LayoutDashboard size={14} />
                  Open overview
                </Button>
              </div>
            </CardContent>
          </Card>

          <Card className="dashboard-card-wide border-border/80 bg-card/95">
            <CardHeader>
              <CardTitle className="dashboard-section-title">
                <Server size={16} />
                Machines
              </CardTitle>
            </CardHeader>
            <CardContent className="server-dashboard-list dashboard-card-list">
              {machines.map((machine) => {
                const managedContainers = visibleContainers(machine.containers);
                const reachable = managedContainers.filter((item) => item.sshReachable).length;

                return (
                  <div className="server-dashboard-row dashboard-card-row" key={machine.id}>
                    <div className="server-dashboard-row-copy">
                      <span className="server-dashboard-row-title">{machine.label}</span>
                      <span className="server-dashboard-row-meta">
                        {machine.host} · {managedContainers.length} container(s) · {reachable} ssh ready
                      </span>
                    </div>

                    <div className="server-dashboard-row-actions">
                      <ConnectionStatusLight state={machine.state} />
                      <Button
                        aria-label={`Edit ${machine.label}`}
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
            <CardContent className="server-dashboard-list dashboard-card-list">
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
                      <ConnectionStatusLight state={container.sshState} title={container.sshState} />
                      {primary ? (
                        <Badge variant="neutral">{primary.label}</Badge>
                      ) : null}
                      <Button
                        aria-label={`Edit ${container.label}`}
                        onClick={() => {
                          onEditAgentTeam(machine.id);
                        }}
                        size="icon"
                        title="Container settings"
                        variant="ghost"
                      >
                        <Settings2 size={14} />
                      </Button>
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
                <div className="server-dashboard-row dashboard-card-row" key={target.id}>
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
        </div>
      </article>
    </section>
  );
}
