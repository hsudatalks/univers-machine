import { Activity, HardDrive, Server, SquareTerminal } from "lucide-react";
import { visibleContainers } from "../lib/container-visibility";
import type { DeveloperTarget, ManagedMachine } from "../types";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

interface ServerDashboardPaneProps {
  onOpenWorkspace: (targetId: string) => void;
  resolveTarget: (targetId: string) => DeveloperTarget | undefined;
  server: ManagedMachine;
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

export function ServerDashboardPane({
  onOpenWorkspace,
  resolveTarget,
  server,
}: ServerDashboardPaneProps) {
  const managedContainers = visibleContainers(server.containers);
  const reachableContainers = managedContainers.filter(
    (container) => container.sshReachable,
  ).length;
  const unreachableContainers = managedContainers.length - reachableContainers;

  return (
    <article className="panel tool-panel dashboard-panel server-dashboard-panel">
      <div className="dashboard-grid">
        <Card className="dashboard-card-hero border-border/80 bg-card/95">
          <CardContent className="dashboard-summary-bar">
            <div className="dashboard-summary-copy">
              <div className="dashboard-summary-item">
                <span className="dashboard-meta-label">Machine host</span>
                <span className="dashboard-meta-value">{server.host}</span>
              </div>
              <div className="dashboard-summary-item">
                <span className="dashboard-meta-label">Inventory</span>
                <span className="dashboard-meta-value">
                  {managedContainers.length} container(s)
                </span>
              </div>
              <div className="dashboard-summary-item">
                <span className="dashboard-meta-label">SSH ready</span>
                <span className="dashboard-meta-value">
                  {reachableContainers} / {managedContainers.length}
                </span>
              </div>
            </div>

            <div className="dashboard-summary-actions">
              <Badge variant={inventoryStateVariant(server.state)}>
                {server.state}
              </Badge>
            </div>
          </CardContent>
        </Card>

        <Card className="border-border/80 bg-card/95">
          <CardHeader>
            <CardTitle className="dashboard-section-title">
              <Activity size={16} />
              Machine status
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="dashboard-copy">{server.message}</p>
            <dl className="dashboard-stats">
              <div>
                <dt>Reachable terminals</dt>
                <dd>{reachableContainers}</dd>
              </div>
              <div>
                <dt>Unavailable terminals</dt>
                <dd>{unreachableContainers}</dd>
              </div>
              <div className="is-wide">
                <dt>Mode</dt>
                <dd>Direct machine shell on the left, container terminals on the right.</dd>
              </div>
            </dl>
          </CardContent>
        </Card>

        <Card className="border-border/80 bg-card/95">
          <CardHeader>
            <CardTitle className="dashboard-section-title">
              <HardDrive size={16} />
              Management focus
            </CardTitle>
          </CardHeader>
          <CardContent>
            <ul className="dashboard-actions-list">
              <li className="dashboard-action-item">
                <span className="dashboard-action-title">Dashboard pane</span>
                <p className="dashboard-copy">
                  Machine inventory, SSH reachability, and a quick read on container health.
                </p>
              </li>
              <li className="dashboard-action-item">
                <span className="dashboard-action-title">Container terminals pane</span>
                <p className="dashboard-copy">
                  Live terminal cards for discovered containers, without leaving the machine context.
                </p>
              </li>
            </ul>
          </CardContent>
        </Card>

        <Card className="dashboard-card-wide border-border/80 bg-card/95">
          <CardHeader>
            <CardTitle className="dashboard-section-title">
              <Server size={16} />
              Containers
            </CardTitle>
          </CardHeader>
            <CardContent className="server-dashboard-list">
            {managedContainers.length ? (
              managedContainers.map((container) => {
                const target = resolveTarget(container.targetId);

                return (
                  <div className="server-dashboard-row" key={container.targetId}>
                    <div className="server-dashboard-row-copy">
                      <span className="server-dashboard-row-title">{container.label}</span>
                      <span className="server-dashboard-row-meta">
                        {container.name} · {container.ipv4 || "no ip"}
                      </span>
                    </div>

                    <div className="server-dashboard-row-actions">
                      <Badge variant={sshStateVariant(container.sshReachable)}>
                        {container.sshState}
                      </Badge>
                      <Button
                        disabled={!target}
                        onClick={() => {
                          if (target) {
                            onOpenWorkspace(target.id);
                          }
                        }}
                        size="sm"
                        variant="ghost"
                      >
                        <SquareTerminal size={14} />
                        Open
                      </Button>
                    </div>
                  </div>
                );
              })
            ) : (
              <p className="dashboard-copy">No managed containers discovered for this machine.</p>
            )}
          </CardContent>
        </Card>
      </div>
    </article>
  );
}
