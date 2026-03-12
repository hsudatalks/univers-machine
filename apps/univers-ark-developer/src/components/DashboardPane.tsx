import { useCallback, useEffect, useState } from "react";
import {
  Bot,
  GitBranch,
  Globe,
  HardDrive,
  Monitor,
  RefreshCw,
} from "lucide-react";
import {
  listenContainerDashboardUpdates,
  executeCommandService,
  restartTmux,
  refreshContainerDashboard,
  startDashboardMonitor,
  stopDashboardMonitor,
} from "../lib/tauri";
import type {
  ContainerDashboard,
  ContainerDashboardUpdate,
  ContainerTmuxSessionInfo,
  DeveloperTarget,
  ServiceStatus,
  TunnelStatus,
} from "../types";
import { tmuxCommandService } from "../lib/target-services";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "./ui/card";
import { Separator } from "./ui/separator";

interface DashboardPaneProps {
  dashboardRefreshSeconds: number;
  primaryBrowserLabel?: string;
  primaryBrowserStatus?: TunnelStatus;
  primaryBrowserUrl?: string;
  serviceStatuses: Record<string, ServiceStatus>;
  target: DeveloperTarget;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

function formatStoragePair(used: number, total: number): string {
  if (!total) return "—";
  return `${(used / 1024 ** 3).toFixed(1)} / ${(total / 1024 ** 3).toFixed(1)} GB`;
}

function formatPercent(used: number, total: number): string {
  if (!total) return "—";
  return `${Math.round((used / total) * 100)}%`;
}

function formatUptime(seconds: number): string {
  if (!seconds) return "—";
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  if (days > 0) {
    return `${days}d ${hours}h`;
  }
  if (hours > 0) {
    return `${hours}h`;
  }
  return `${Math.max(Math.floor(seconds / 60), 1)}m`;
}

function tunnelBadgeVariant(
  state: string | undefined,
): "neutral" | "success" | "warning" | "destructive" {
  switch (state) {
    case "running":
    case "ready":
      return "success";
    case "starting":
      return "warning";
    case "error":
    case "stopped":
      return "destructive";
    default:
      return "neutral";
  }
}

function serviceBadgeVariant(
  state: string | undefined,
): "neutral" | "success" | "warning" | "destructive" {
  switch (state) {
    case "running":
    case "ready":
    case "healthy":
    case "embedded":
      return "success";
    case "starting":
    case "unknown":
      return "warning";
    case "down":
    case "error":
    case "failed":
      return "destructive";
    default:
      return "neutral";
  }
}

function agentBadgeVariant(
  state: string | undefined,
): "neutral" | "success" | "warning" | "destructive" {
  switch (state) {
    case "claude":
    case "codex":
      return "success";
    case "mixed":
      return "warning";
    case "agent":
      return "warning";
    case "unknown":
      return "neutral";
    default:
      return "neutral";
  }
}

function formatRefreshLabel(seconds: number): string {
  if (seconds <= 0) {
    return "manual";
  }

  if (seconds < 60) {
    return `${seconds}s`;
  }

  if (seconds % 60 === 0) {
    return `${seconds / 60}m`;
  }

  return `${seconds}s`;
}

export function DashboardPane({
  dashboardRefreshSeconds,
  primaryBrowserLabel,
  primaryBrowserStatus,
  primaryBrowserUrl,
  serviceStatuses,
  target,
}: DashboardPaneProps) {
  const [dashboard, setDashboard] = useState<ContainerDashboard | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isRestartingTmux, setIsRestartingTmux] = useState(false);
  const [tmuxActionError, setTmuxActionError] = useState<string | null>(null);
  const [lastUpdatedAt, setLastUpdatedAt] = useState<number | null>(null);
  const applyUpdate = useCallback(
    (payload: ContainerDashboardUpdate) => {
      if (payload.targetId !== target.id) {
        return;
      }

      setError(payload.error ?? null);
      setDashboard(payload.dashboard);
      setLastUpdatedAt(payload.refreshedAtMs || Date.now());
      setIsLoading(false);
      setIsRefreshing(false);
    },
    [target.id],
  );

  useEffect(() => {
    let isDisposed = false;
    let unlisten: (() => void) | undefined;

    void listenContainerDashboardUpdates((payload) => {
      if (!isDisposed) {
        applyUpdate(payload);
      }
    }).then((dispose) => {
      if (isDisposed) {
        dispose();
        return;
      }
      unlisten = dispose;
    });

    void startDashboardMonitor(target.id, dashboardRefreshSeconds);

    return () => {
      isDisposed = true;
      unlisten?.();
      void stopDashboardMonitor(target.id);
    };
  }, [applyUpdate, dashboardRefreshSeconds, target.id]);

  const project = dashboard?.project;
  const runtime = dashboard?.runtime;
  const agent = dashboard?.agent;
  const tmux = dashboard?.tmux;
  const declaredTmuxService = tmuxCommandService(target);
  const diskPercent = formatPercent(
    runtime?.diskUsedBytes ?? 0,
    runtime?.diskTotalBytes ?? 0,
  );
  const memoryPercent = formatPercent(
    runtime?.memoryUsedBytes ?? 0,
    runtime?.memoryTotalBytes ?? 0,
  );

  const workspaceSignals = [
    {
      label: "Repo",
      value: project?.repoFound ? "ready" : "missing",
      variant: project?.repoFound ? "success" : "warning",
      detail:
        project?.projectPath ??
        target.workspace.projectPath ??
        target.workspace.filesRoot ??
        "Unavailable",
    },
    {
      label: "Branch",
      value: project?.branch ?? "unavailable",
      variant: "neutral",
      detail: project?.headSummary ?? "No recent commit found",
    },
    {
      label: "Working tree",
      value: project?.isDirty ? "dirty" : "clean",
      variant: project?.isDirty ? "warning" : "success",
      detail: `${project?.changedFiles ?? 0} changed file(s)`,
    },
  ] as const;

  const serviceSignals = target.services
    .filter((service) => service.kind !== "command")
    .map((service) => {
      const key = `${target.id}::${service.id}`;
      const registryStatus = serviceStatuses[key];
      const dashboardStatus = dashboard?.services.find(
        (candidate) => candidate.id === service.id,
      );
      const fallbackUrl =
        service.web?.localUrl ||
        service.web?.remoteUrl ||
        service.endpoint?.url ||
        (service.endpoint
          ? `tcp://${service.endpoint.host || "127.0.0.1"}:${service.endpoint.port}`
          : null);
      const status = registryStatus?.state ?? dashboardStatus?.status ?? "unknown";
      const detail =
        registryStatus?.message ??
        dashboardStatus?.detail ??
        (service.description || "No runtime status yet.");
      const url =
        registryStatus?.localUrl ?? dashboardStatus?.url ?? fallbackUrl ?? null;

      return {
        id: service.id,
        label: service.label,
        status,
        detail,
        url,
        variant: serviceBadgeVariant(status),
      };
    });

  const tmuxSessionRows: ContainerTmuxSessionInfo[] = tmux?.sessions ?? [];
  const defaultTmuxSessions = tmuxSessionRows.filter(
    (session) => session.server === "default",
  );
  const containerTmuxSessions = tmuxSessionRows.filter(
    (session) => session.server === "container",
  );

  return (
    <article className="panel tool-panel dashboard-panel">
      <div className="dashboard-grid">
        <Card className="dashboard-card-hero border-border/80 bg-card/95">
          <CardContent className="dashboard-summary-bar dashboard-card-content">
            <div className="dashboard-summary-copy">
              <div className="dashboard-summary-item">
                <span className="dashboard-meta-label">Project root</span>
                <span className="dashboard-meta-value">
                  {project?.projectPath ??
                    target.workspace.projectPath ??
                    target.workspace.filesRoot ??
                    "Unavailable"}
                </span>
              </div>
              <div className="dashboard-summary-item">
                <span className="dashboard-meta-label">
                  {primaryBrowserLabel ? `${primaryBrowserLabel} URL` : "Primary URL"}
                </span>
                <span className="dashboard-meta-value">
                  {primaryBrowserUrl ?? "Unavailable"}
                </span>
              </div>
              <div className="dashboard-summary-item">
                <span className="dashboard-meta-label">Last updated</span>
                <span className="dashboard-meta-value">
                  {lastUpdatedAt
                    ? new Date(lastUpdatedAt).toLocaleTimeString([], {
                        hour: "2-digit",
                        minute: "2-digit",
                        second: "2-digit",
                      })
                    : "Waiting…"}
                </span>
              </div>
            </div>
            <div className="dashboard-summary-actions">
              <Badge variant={tunnelBadgeVariant(primaryBrowserStatus?.state)}>
                {primaryBrowserStatus?.state ?? "idle"}
              </Badge>
              <Button
                aria-label={isRefreshing ? "Refreshing dashboard" : "Refresh dashboard now"}
                disabled={isRefreshing}
                onClick={() => {
                  setIsRefreshing(true);
                  void refreshContainerDashboard(target.id);
                }}
                size="icon"
                title={
                  isRefreshing
                    ? "Refreshing dashboard…"
                    : `Refresh dashboard (${dashboardRefreshSeconds}s auto)`
                }
                variant="ghost"
              >
                <RefreshCw className={isRefreshing ? "animate-spin" : ""} size={14} />
              </Button>
              <Badge variant="neutral">{formatRefreshLabel(dashboardRefreshSeconds)}</Badge>
            </div>
          </CardContent>
        </Card>

        <Card className="border-border/80 bg-card/95">
          <CardHeader className="pb-3">
            <CardTitle className="dashboard-section-title">
              <GitBranch size={14} />
              Git
            </CardTitle>
          </CardHeader>
          <Separator />
          <CardContent className="pt-4">
            {isLoading ? (
              <p className="dashboard-copy">Loading git state…</p>
            ) : error ? (
              <p className="dashboard-copy">{error}</p>
            ) : (
              <div className="grid gap-3 md:grid-cols-2">
                {workspaceSignals.map((item) => (
                  <div className="dashboard-signal" key={item.label}>
                    <div className="dashboard-signal-header">
                      <span className="dashboard-meta-label">{item.label}</span>
                      <Badge variant={item.variant}>{item.value}</Badge>
                    </div>
                    <div className="dashboard-meta-value">{item.detail}</div>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="border-border/80 bg-card/95">
          <CardHeader className="pb-3">
            <CardTitle className="dashboard-section-title">
              <Globe size={14} />
              Service
            </CardTitle>
          </CardHeader>
          <Separator />
          <CardContent className="pt-4">
            {serviceSignals.length === 0 && isLoading ? (
              <p className="dashboard-copy">Loading service state…</p>
            ) : serviceSignals.length === 0 && error ? (
              <p className="dashboard-copy">{error}</p>
            ) : serviceSignals.length === 0 ? (
              <p className="dashboard-copy">No runtime services detected.</p>
            ) : (
              <div className="grid gap-3 md:grid-cols-2">
                {serviceSignals.map((item) => (
                  <div className="dashboard-signal" key={item.id}>
                    <div className="dashboard-signal-header">
                      <span className="dashboard-meta-label">{item.label}</span>
                      <Badge variant={item.variant}>{item.status}</Badge>
                    </div>
                    <div className="dashboard-meta-value">{item.detail}</div>
                    {item.url ? (
                      <div className="dashboard-copy dashboard-service-url">{item.url}</div>
                    ) : null}
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="border-border/80 bg-card/95">
          <CardHeader className="pb-3">
            <CardTitle className="dashboard-section-title">
              <HardDrive size={14} />
              Container
            </CardTitle>
          </CardHeader>
          <Separator />
          <CardContent className="pt-4">
            {isLoading ? (
              <p className="dashboard-copy">Loading container info…</p>
            ) : error ? (
              <p className="dashboard-copy">{error}</p>
            ) : (
              <dl className="dashboard-stats">
                <div>
                  <dt>Host</dt>
                  <dd>{runtime?.hostname ?? target.host}</dd>
                </div>
                <div>
                  <dt>Uptime</dt>
                  <dd>{formatUptime(runtime?.uptimeSeconds ?? 0)}</dd>
                </div>
                <div>
                  <dt>Memory</dt>
                  <dd>
                    {formatBytes(runtime?.memoryUsedBytes ?? 0)} /{" "}
                    {formatBytes(runtime?.memoryTotalBytes ?? 0)} ({memoryPercent})
                  </dd>
                </div>
                <div>
                  <dt>Disk</dt>
                  <dd>
                    {formatStoragePair(
                      runtime?.diskUsedBytes ?? 0,
                      runtime?.diskTotalBytes ?? 0,
                    )}{" "}
                    ({diskPercent})
                  </dd>
                </div>
                <div>
                  <dt>Processes</dt>
                  <dd>{runtime?.processCount ?? 0}</dd>
                </div>
                <div>
                  <dt>Load 1m</dt>
                  <dd>{runtime ? runtime.loadAverage1m.toFixed(2) : "0.00"}</dd>
                </div>
                <div>
                  <dt>Load 5m</dt>
                  <dd>{runtime ? runtime.loadAverage5m.toFixed(2) : "0.00"}</dd>
                </div>
                <div>
                  <dt>Load 15m</dt>
                  <dd>{runtime ? runtime.loadAverage15m.toFixed(2) : "0.00"}</dd>
                </div>
              </dl>
            )}
          </CardContent>
        </Card>

        <Card className="border-border/80 bg-card/95">
          <CardHeader className="pb-3">
            <CardTitle className="dashboard-section-title">
              <Bot size={14} />
              Agent
            </CardTitle>
          </CardHeader>
          <Separator />
          <CardContent className="pt-4">
            {isLoading ? (
              <p className="dashboard-copy">Loading agent info…</p>
            ) : error ? (
              <p className="dashboard-copy">{error}</p>
            ) : (
              <dl className="dashboard-stats">
                <div>
                  <dt>Active</dt>
                  <dd className="dashboard-inline-value">
                    <span>{agent?.activeAgent ?? "unknown"}</span>
                    <Badge variant={agentBadgeVariant(agent?.activeAgent)}>
                      {agent?.source ?? "none"}
                    </Badge>
                  </dd>
                </div>
                <div>
                  <dt>Last activity</dt>
                  <dd>{agent?.lastActivity ?? "No recent agent signal"}</dd>
                </div>
                <div className="is-wide">
                  <dt>Latest report</dt>
                  <dd>
                    {agent?.latestReport
                      ? `${agent.latestReport}${
                          agent.latestReportUpdatedAt
                            ? ` · ${agent.latestReportUpdatedAt}`
                            : ""
                        }`
                      : "No recent agent report"}
                  </dd>
                </div>
              </dl>
            )}
          </CardContent>
        </Card>

        <Card className="dashboard-card-wide border-border/80 bg-card/95">
          <CardHeader className="pb-3">
            <div className="dashboard-card-header-row">
              <CardTitle className="dashboard-section-title">
                <Monitor size={14} />
                Tmux
              </CardTitle>
              <Button
                aria-label={isRestartingTmux ? "Restarting tmux" : "Restart tmux"}
                disabled={isRestartingTmux}
                onClick={() => {
                  setTmuxActionError(null);
                  setIsRestartingTmux(true);
                  const restartPromise = declaredTmuxService
                    ? executeCommandService(target.id, declaredTmuxService.id, "restart")
                    : restartTmux(target.id);

                  void restartPromise
                    .then(() => refreshContainerDashboard(target.id))
                    .catch((error: unknown) => {
                      setTmuxActionError(
                        error instanceof Error ? error.message : String(error),
                      );
                    })
                    .finally(() => {
                      setIsRestartingTmux(false);
                    });
                }}
                size="icon"
                title={isRestartingTmux ? "Restarting tmux…" : "Restart tmux"}
                variant="ghost"
              >
                <RefreshCw className={isRestartingTmux ? "animate-spin" : ""} size={14} />
              </Button>
            </div>
          </CardHeader>
          <Separator />
          <CardContent className="pt-4">
            {isLoading ? (
              <p className="dashboard-copy">Loading tmux info…</p>
            ) : error ? (
              <p className="dashboard-copy">{error}</p>
            ) : !tmux?.installed ? (
              <p className="dashboard-copy">tmux is not installed in this container.</p>
            ) : !tmux.serverRunning ? (
              <p className="dashboard-copy">tmux is installed, but no server is currently running.</p>
            ) : (
              <div className="dashboard-tmux">
                {tmuxActionError ? (
                  <p className="dashboard-copy dashboard-error-copy">{tmuxActionError}</p>
                ) : null}
                {defaultTmuxSessions.length > 0 ? (
                  <section className="dashboard-tmux-group">
                    <div className="dashboard-group-label">Default</div>
                    <div className="dashboard-tmux-sessions">
                      {defaultTmuxSessions.map((session) => (
                        <div className="dashboard-signal" key={`${session.server}:${session.name}`}>
                          <div className="dashboard-signal-header">
                            <span className="dashboard-meta-value">{session.name}</span>
                            <Badge variant={session.attached ? "success" : "neutral"}>
                              {session.attached ? "attached" : "detached"}
                            </Badge>
                          </div>
                          <div className="dashboard-copy">
                            {session.windows} window{session.windows === 1 ? "" : "s"}
                          </div>
                          <div className="dashboard-meta-value">
                            {session.activeCommand ?? "No active command"}
                          </div>
                        </div>
                      ))}
                    </div>
                  </section>
                ) : null}
                {containerTmuxSessions.length > 0 ? (
                  <section className="dashboard-tmux-group">
                    <div className="dashboard-group-label">Container</div>
                    <div className="dashboard-tmux-sessions">
                      {containerTmuxSessions.map((session) => (
                        <div className="dashboard-signal" key={`${session.server}:${session.name}`}>
                          <div className="dashboard-signal-header">
                            <span className="dashboard-meta-value">{session.name}</span>
                            <Badge variant={session.attached ? "success" : "neutral"}>
                              {session.attached ? "attached" : "detached"}
                            </Badge>
                          </div>
                          <div className="dashboard-copy">
                            {session.windows} window{session.windows === 1 ? "" : "s"}
                          </div>
                          <div className="dashboard-meta-value">
                            {session.activeCommand ?? "No active command"}
                          </div>
                        </div>
                      ))}
                    </div>
                  </section>
                ) : null}
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </article>
  );
}
