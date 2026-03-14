import { Server, SquareTerminal } from "lucide-react";
import { visibleContainers } from "../lib/container-visibility";
import type { DeveloperTarget, ManagedMachine } from "../types";
import { ConnectionStatusLight } from "./ConnectionStatusLight";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

interface ServerDashboardPaneProps {
  onOpenWorkspace: (targetId: string) => void;
  resolveTarget: (targetId: string) => DeveloperTarget | undefined;
  server: ManagedMachine;
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

export function ServerDashboardPane({
  onOpenWorkspace,
  resolveTarget,
  server,
}: ServerDashboardPaneProps) {
  const managedContainers = visibleContainers(server.containers);
  const reachableContainers = managedContainers.filter(
    (container) => container.sshReachable,
  ).length;
  const reachableSshEndpoints = reachableContainers + (isSshReadyState(server.state) ? 1 : 0);
  const totalSshEndpoints = managedContainers.length + 1;

  return (
    <article className="panel tool-panel dashboard-panel server-dashboard-panel">
      <div className="dashboard-grid">
        <Card className="dashboard-card-hero border-border/80 bg-card/95">
          <CardContent className="dashboard-summary-bar dashboard-card-content">
            <div className="dashboard-summary-copy">
              <div className="dashboard-summary-item">
                <span className="dashboard-meta-label">Provider host</span>
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
                  {reachableSshEndpoints} / {totalSshEndpoints}
                </span>
              </div>
            </div>

            <div className="dashboard-summary-actions">
              <ConnectionStatusLight state={server.state} />
            </div>
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
                      <ConnectionStatusLight state={container.sshState} title={container.sshState} />
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
              <p className="dashboard-copy">No managed containers discovered for this provider.</p>
            )}
          </CardContent>
        </Card>
      </div>
    </article>
  );
}
