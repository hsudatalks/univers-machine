import { useCallback, useEffect, useRef, useState } from "react";
import { Activity, Box, RefreshCw, SquareTerminal } from "lucide-react";
import type { DockerContainerStats, ManagedContainer, ManagedServer } from "../types";
import { getDockerStats } from "../lib/tauri";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

interface DockerServerPageProps {
  onOpenWorkspace: (targetId: string) => void;
  pageVisible: boolean;
  server: ManagedServer;
}

function statusVariant(status: string): "success" | "warning" | "destructive" | "neutral" {
  if (status === "running") return "success";
  if (status === "paused") return "warning";
  if (status === "exited" || status === "stopped") return "destructive";
  return "neutral";
}

function serverStateVariant(state: string): "success" | "warning" | "destructive" | "neutral" {
  if (state === "ready") return "success";
  if (state === "degraded" || state === "empty") return "warning";
  if (state === "stopped" || state === "error") return "destructive";
  return "neutral";
}

function ContainerStatsRow({
  container,
  onOpen,
}: {
  container: ManagedContainer;
  onOpen: () => void;
}) {
  const [stats, setStats] = useState<DockerContainerStats | null>(null);
  const [loading, setLoading] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchStats = useCallback(async () => {
    setLoading(true);
    try {
      const s = await getDockerStats(container.name);
      setStats(s);
    } catch {
      // ignore — Docker may not be running
    } finally {
      setLoading(false);
    }
  }, [container.name]);

  useEffect(() => {
    void fetchStats();
    intervalRef.current = setInterval(() => void fetchStats(), 5000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [fetchStats]);

  const s = stats;

  return (
    <div className="server-dashboard-row docker-stats-row">
      <div className="server-dashboard-row-copy">
        <span className="server-dashboard-row-title">{container.label}</span>
        <span className="server-dashboard-row-meta">{container.name}</span>
      </div>

      <div className="docker-stats-cells">
        <div className="docker-stat-cell">
          <span className="docker-stat-label">Status</span>
          <Badge variant={statusVariant(s?.status ?? container.status)}>
            {s?.status ?? container.status}
          </Badge>
        </div>
        <div className="docker-stat-cell">
          <span className="docker-stat-label">CPU</span>
          <span className="docker-stat-value">{s?.cpuPercent ?? "--"}</span>
        </div>
        <div className="docker-stat-cell">
          <span className="docker-stat-label">Memory</span>
          <span className="docker-stat-value">{s ? `${s.memUsage} (${s.memPercent})` : "--"}</span>
        </div>
        <div className="docker-stat-cell">
          <span className="docker-stat-label">Net I/O</span>
          <span className="docker-stat-value">{s?.netIo ?? "--"}</span>
        </div>
        <div className="docker-stat-cell">
          <span className="docker-stat-label">PIDs</span>
          <span className="docker-stat-value">{s?.pids ?? "--"}</span>
        </div>
      </div>

      <div className="server-dashboard-row-actions">
        {loading && <RefreshCw className="docker-stat-spinner" size={12} />}
        <Button
          disabled={!container.sshReachable}
          onClick={onOpen}
          size="sm"
          variant="ghost"
        >
          <SquareTerminal size={14} />
          Terminal
        </Button>
      </div>
    </div>
  );
}

export function DockerServerPage({
  onOpenWorkspace,
  pageVisible: _pageVisible,
  server,
}: DockerServerPageProps) {
  const running = server.containers.filter((c) => c.sshReachable).length;

  return (
    <>
      <header className="content-header">
        <div className="content-header-copy">
          <span className="panel-title">Docker</span>
          <h1 className="content-title content-title-container">{server.label}</h1>
          <p className="panel-description">{server.description}</p>
        </div>
      </header>

      <div className="content-meta-row">
        <span className="content-chip">{server.containers.length} container(s)</span>
        <span className="content-chip">{running} running</span>
        <Badge variant={serverStateVariant(server.state)}>{server.state}</Badge>
      </div>

      <section className="page-section">
        <div className="dashboard-grid">
          <Card className="dashboard-card-hero border-border/80 bg-card/95">
            <CardContent className="dashboard-summary-bar">
              <div className="dashboard-summary-copy">
                <div className="dashboard-summary-item">
                  <span className="dashboard-meta-label">Host</span>
                  <span className="dashboard-meta-value">localhost (Docker Desktop)</span>
                </div>
                <div className="dashboard-summary-item">
                  <span className="dashboard-meta-label">Containers</span>
                  <span className="dashboard-meta-value">{server.containers.length}</span>
                </div>
                <div className="dashboard-summary-item">
                  <span className="dashboard-meta-label">Running</span>
                  <span className="dashboard-meta-value">{running} / {server.containers.length}</span>
                </div>
              </div>
              <div className="dashboard-summary-actions">
                <Badge variant={serverStateVariant(server.state)}>{server.state}</Badge>
              </div>
            </CardContent>
          </Card>

          <Card className="border-border/80 bg-card/95">
            <CardHeader>
              <CardTitle className="dashboard-section-title">
                <Activity size={16} />
                Status
              </CardTitle>
            </CardHeader>
            <CardContent>
              <p className="dashboard-copy">{server.message}</p>
              <dl className="dashboard-stats">
                <div>
                  <dt>Running</dt>
                  <dd>{running}</dd>
                </div>
                <div>
                  <dt>Stopped</dt>
                  <dd>{server.containers.length - running}</dd>
                </div>
                <div className="is-wide">
                  <dt>Stats refresh</dt>
                  <dd>Every 5 seconds (requires Docker Desktop running).</dd>
                </div>
              </dl>
            </CardContent>
          </Card>

          <Card className="dashboard-card-wide border-border/80 bg-card/95">
            <CardHeader>
              <CardTitle className="dashboard-section-title">
                <Box size={16} />
                Containers
              </CardTitle>
            </CardHeader>
            <CardContent className="server-dashboard-list">
              {server.containers.length === 0 ? (
                <p className="dashboard-copy">No Docker containers added yet.</p>
              ) : (
                server.containers.map((container) => (
                  <ContainerStatsRow
                    container={container}
                    key={container.targetId}
                    onOpen={() => onOpenWorkspace(container.targetId)}
                  />
                ))
              )}
            </CardContent>
          </Card>
        </div>
      </section>
    </>
  );
}
