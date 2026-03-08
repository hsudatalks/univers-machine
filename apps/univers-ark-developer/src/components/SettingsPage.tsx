import { useEffect, useMemo, useState } from "react";
import {
  listBrowserFrameSnapshots,
  preloadBrowserFrames,
} from "../lib/browser-cache";
import { ensureTunnel, restartAllTunnels, restartTunnel } from "../lib/tauri";
import type {
  AppSettings,
  DeveloperTarget,
  ManagedServer,
  ThemeMode,
  TunnelStatus,
} from "../types";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { ServerDialog } from "./ServerDialog";

type SettingsTab = "appearance" | "configuration" | "servers" | "tunnels" | "iframes";

interface SettingsPageProps {
  appSettings: AppSettings;
  configPath: string;
  onDashboardRefreshChange: (seconds: number) => void;
  onConfigSaved: () => void;
  onThemeModeChange: (themeMode: ThemeMode) => void;
  resolvedTheme: "light" | "dark";
  servers: ManagedServer[];
  targets: DeveloperTarget[];
  tunnelStatuses: Record<string, TunnelStatus>;
}

function surfaceKey(targetId: string, surfaceId: string): string {
  return `${targetId}::${surfaceId}`;
}

function formatTimestamp(timestamp: number): string {
  if (!timestamp) {
    return "Never";
  }

  return new Date(timestamp).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function badgeVariantForState(state: string | undefined): "neutral" | "success" | "warning" | "destructive" {
  switch (state) {
    case "running":
    case "ready":
      return "success";
    case "starting":
    case "pending":
      return "warning";
    case "error":
    case "stopped":
      return "destructive";
    default:
      return "neutral";
  }
}

function badgeLabelForIframeState(
  state: "idle" | "loading" | "loaded" | "error",
): string {
  switch (state) {
    case "idle":
      return "idle";
    case "loading":
      return "loading";
    case "loaded":
      return "loaded";
    case "error":
      return "error";
    default:
      return state;
  }
}

function isReadyTunnelState(state: string | undefined): boolean {
  return state === "running" || state === "direct";
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export function SettingsPage({
  appSettings,
  configPath,
  onDashboardRefreshChange,
  onConfigSaved,
  onThemeModeChange,
  resolvedTheme,
  servers,
  targets,
  tunnelStatuses,
}: SettingsPageProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("appearance");
  const [selectedServer, setSelectedServer] = useState<ManagedServer | null>(null);
  const [restartingTunnelKey, setRestartingTunnelKey] = useState<string>("");
  const [isRestartingAllTunnels, setIsRestartingAllTunnels] = useState(false);
  const [isPreloadingIframes, setIsPreloadingIframes] = useState(false);
  const [preloadSummary, setPreloadSummary] = useState<string>("");
  const [iframeSnapshots, setIframeSnapshots] = useState(listBrowserFrameSnapshots());

  useEffect(() => {
    if (activeTab !== "iframes") {
      return;
    }

    setIframeSnapshots(listBrowserFrameSnapshots());

    const intervalId = window.setInterval(() => {
      setIframeSnapshots(listBrowserFrameSnapshots());
    }, 1000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [activeTab]);

  const tunnelEntries = useMemo(() => {
    return targets.flatMap((target) =>
      target.surfaces
        .filter(
          (surface) =>
            surface.id === "development" && surface.tunnelCommand.trim(),
        )
        .map((surface) => {
          const key = surfaceKey(target.id, surface.id);
          const status = tunnelStatuses[key];

          return {
            cacheKey: key,
            status,
            surface,
            target,
          };
        }),
    );
  }, [targets, tunnelStatuses]);

  const restartTunnelEntry = async (targetId: string, surfaceId: string, cacheKey: string) => {
    setRestartingTunnelKey(cacheKey);

    try {
      await restartTunnel(targetId, surfaceId);
    } finally {
      setRestartingTunnelKey("");
    }
  };

  const restartAllTunnelEntries = async () => {
    setIsRestartingAllTunnels(true);

    try {
      await restartAllTunnels(
        tunnelEntries.map(({ target, surface }) => ({
          targetId: target.id,
          surfaceId: surface.id,
        })),
      );
    } finally {
      setIsRestartingAllTunnels(false);
    }
  };

  const preloadAllIframes = async () => {
    const developmentSurfaces = targets.flatMap((target) =>
      target.surfaces
        .filter((surface) => surface.id === "development" && surface.localUrl.trim())
        .map((surface) => ({ surface, target })),
    );

    if (developmentSurfaces.length === 0) {
      setPreloadSummary("No development browser surfaces available.");
      return;
    }

    setIsPreloadingIframes(true);
    setPreloadSummary(`Loading 0 / ${developmentSurfaces.length} iframe(s)…`);

    let loadedCount = 0;
    let failedCount = 0;

    const updateProgress = () => {
      const finishedCount = loadedCount + failedCount;
      if (finishedCount < developmentSurfaces.length) {
        setPreloadSummary(
          `Loading ${loadedCount} / ${developmentSurfaces.length} iframe(s)…`,
        );
        return;
      }

      setPreloadSummary(
        failedCount > 0
          ? `Loaded ${loadedCount} iframe(s), ${failedCount} failed.`
          : `Loaded ${loadedCount} iframe(s).`,
      );
      setIsPreloadingIframes(false);
    };

    void Promise.allSettled(
      developmentSurfaces.map(async ({ surface, target }) => {
        let status = await ensureTunnel(target.id, surface.id);
        const deadline = Date.now() + 15_000;

        while (!isReadyTunnelState(status.state) && Date.now() < deadline) {
          await sleep(300);
          status = await ensureTunnel(target.id, surface.id);
        }

        if (!isReadyTunnelState(status.state)) {
          throw new Error(
            status.message || `Tunnel did not become ready for ${target.label}.`,
          );
        }

        preloadBrowserFrames([
          {
            cacheKey: surfaceKey(target.id, surface.id),
            frameVersion: 0,
            src: surface.localUrl,
            title: `${target.label} ${surface.label}`,
          },
        ]);

        loadedCount += 1;
        setIframeSnapshots(listBrowserFrameSnapshots());
        updateProgress();
      }),
    ).then((results) => {
      failedCount = results.filter((result) => result.status === "rejected").length;
      setIframeSnapshots(listBrowserFrameSnapshots());
      updateProgress();
    });
  };

  return (
    <div className="settings-page">
      <header className="panel-header settings-header">
        <div className="settings-header-copy">
          <span className="panel-title">Settings</span>
          <p className="panel-description settings-description">
            Application configuration, server inventory, tunnels, and browser cache
          </p>
        </div>
      </header>

      <Tabs
        className="settings-tabs"
        onValueChange={(value) => setActiveTab(value as SettingsTab)}
        value={activeTab}
      >
        <TabsList className="settings-tab-bar" aria-label="Settings sections">
          {(
            [
              ["appearance", "Appearance"],
              ["configuration", "Configuration"],
              ["servers", "Servers"],
              ["tunnels", "Tunnels"],
              ["iframes", "Iframes"],
            ] as Array<[SettingsTab, string]>
          ).map(([tab, label]) => (
            <TabsTrigger className="settings-tab" key={tab} value={tab}>
              {label}
            </TabsTrigger>
          ))}
        </TabsList>

        <div className="settings-body">
          <TabsContent className="settings-tab-panel" value="appearance">
            <section className="settings-section">
            <h3 className="settings-section-title">Appearance</h3>
            <div className="settings-field">
              <label className="settings-label">Theme</label>
              <div className="settings-option-group" role="radiogroup" aria-label="Theme mode">
                {(
                  [
                    ["system", "System"],
                    ["light", "Light"],
                    ["dark", "Dark"],
                  ] as Array<[ThemeMode, string]>
                ).map(([themeMode, label]) => (
                  <Button
                    aria-checked={appSettings.themeMode === themeMode}
                    className="settings-option-button"
                    key={themeMode}
                    onClick={() => onThemeModeChange(themeMode)}
                    role="radio"
                    size="sm"
                    variant={appSettings.themeMode === themeMode ? "default" : "outline"}
                  >
                    {label}
                  </Button>
                ))}
              </div>
            </div>
            <div className="settings-field">
              <label className="settings-label">Resolved theme</label>
              <span className="settings-value">{resolvedTheme}</span>
            </div>
            <div className="settings-field">
              <label className="settings-label">Dashboard refresh</label>
              <div className="settings-option-group" role="radiogroup" aria-label="Dashboard refresh interval">
                {(
                  [
                    [0, "Off"],
                    [15, "15s"],
                    [30, "30s"],
                    [60, "60s"],
                    [300, "5m"],
                  ] as Array<[number, string]>
                ).map(([seconds, label]) => (
                  <Button
                    aria-checked={appSettings.dashboardRefreshSeconds === seconds}
                    className="settings-option-button"
                    key={seconds}
                    onClick={() => onDashboardRefreshChange(seconds)}
                    role="radio"
                    size="sm"
                    variant={appSettings.dashboardRefreshSeconds === seconds ? "default" : "outline"}
                  >
                    {label}
                  </Button>
                ))}
              </div>
            </div>
            </section>
          </TabsContent>

          <TabsContent className="settings-tab-panel" value="configuration">
            <section className="settings-section">
            <h3 className="settings-section-title">Configuration</h3>
            <div className="settings-field">
              <label className="settings-label">Config file path</label>
              <span className="settings-value settings-value-mono">{configPath}</span>
            </div>
            <div className="settings-field">
              <label className="settings-label">Target count</label>
              <span className="settings-value">{targets.length} target(s)</span>
            </div>
            </section>
          </TabsContent>

          <TabsContent className="settings-tab-panel" value="servers">
            <section className="settings-section">
            <h3 className="settings-section-title">
              Servers
              <span className="settings-count">{servers.length}</span>
            </h3>
            {servers.length === 0 ? (
              <p className="settings-empty">No servers configured.</p>
            ) : (
              <div className="settings-server-list">
                {servers.map((server) => (
                  <button
                    className="settings-server-card"
                    key={server.id}
                    onClick={() => setSelectedServer(server)}
                    type="button"
                  >
                    <div className="settings-server-header">
                      <span className="settings-server-label">{server.label}</span>
                      <Badge variant={badgeVariantForState(server.state)}>
                        {server.state}
                      </Badge>
                    </div>
                    <div className="settings-server-meta">
                      <span className="settings-server-host">{server.host}</span>
                      {server.description ? (
                        <span className="settings-server-desc">{server.description}</span>
                      ) : null}
                    </div>
                    <div className="settings-server-footer">
                      <span className="settings-server-containers">
                        {server.containers.length} container(s)
                      </span>
                      {server.message ? (
                        <span className="settings-server-message">{server.message}</span>
                      ) : null}
                    </div>
                  </button>
                ))}
              </div>
            )}
            </section>
          </TabsContent>

          <TabsContent className="settings-tab-panel" value="tunnels">
            <section className="settings-section">
            <div className="settings-section-heading">
              <h3 className="settings-section-title">
                Tunnel sessions
                <span className="settings-count">{tunnelEntries.length}</span>
              </h3>
              <Button
                disabled={tunnelEntries.length === 0 || isRestartingAllTunnels}
                onClick={() => void restartAllTunnelEntries()}
                size="sm"
                variant="outline"
              >
                {isRestartingAllTunnels ? "Restarting all…" : "Restart all"}
              </Button>
            </div>
            {tunnelEntries.length === 0 ? (
              <p className="settings-empty">No managed tunnel surfaces configured.</p>
            ) : (
              <div className="settings-runtime-list">
                {tunnelEntries.map(({ cacheKey, status, surface, target }) => (
                  <article className="settings-runtime-card" key={cacheKey}>
                    <div className="settings-runtime-header">
                      <div className="settings-runtime-copy">
                        <span className="settings-runtime-label">
                          {target.label} · {surface.label}
                        </span>
                        <span className="settings-runtime-key">{cacheKey}</span>
                      </div>
                      <div className="settings-runtime-actions">
                        <Badge variant={badgeVariantForState(status?.state)}>
                          {status?.state ?? "stopped"}
                        </Badge>
                        <Button
                          disabled={restartingTunnelKey === cacheKey}
                          onClick={() => void restartTunnelEntry(target.id, surface.id, cacheKey)}
                          size="sm"
                          variant="outline"
                        >
                          {restartingTunnelKey === cacheKey ? "Restarting…" : "Restart"}
                        </Button>
                      </div>
                    </div>
                    <div className="settings-runtime-grid">
                      <span className="settings-runtime-row">
                        <strong>Local</strong> {surface.localUrl}
                      </span>
                      <span className="settings-runtime-row">
                        <strong>Remote</strong> {surface.remoteUrl}
                      </span>
                      <span className="settings-runtime-row">
                        <strong>Message</strong> {status?.message ?? "No runtime status yet."}
                      </span>
                    </div>
                  </article>
                ))}
              </div>
            )}
            </section>
          </TabsContent>

          <TabsContent className="settings-tab-panel" value="iframes">
            <section className="settings-section">
            <div className="settings-section-heading">
              <h3 className="settings-section-title">
                Iframe cache
                <span className="settings-count">{iframeSnapshots.length}</span>
              </h3>
              <Button
                disabled={isPreloadingIframes}
                onClick={() => void preloadAllIframes()}
                size="sm"
                variant="outline"
              >
                {isPreloadingIframes ? "Loading all…" : "Load all"}
              </Button>
            </div>
            {preloadSummary ? (
              <p className="settings-description">{preloadSummary}</p>
            ) : null}
            {iframeSnapshots.length === 0 ? (
              <p className="settings-empty">No retained browser iframes in memory.</p>
            ) : (
              <div className="settings-runtime-list">
                {iframeSnapshots.map((frame) => (
                  <article className="settings-runtime-card" key={frame.cacheKey}>
                    <div className="settings-runtime-header">
                      <div className="settings-runtime-copy">
                        <span className="settings-runtime-label">{frame.title || "Untitled frame"}</span>
                        <span className="settings-runtime-key">{frame.cacheKey}</span>
                      </div>
                      <div className="settings-runtime-actions">
                        <Badge variant={badgeVariantForState(frame.hasOwner ? "running" : "pending")}>
                          {frame.hasOwner ? "attached" : "parked"}
                        </Badge>
                        <Badge variant={badgeVariantForState(frame.sessionState)}>
                          {badgeLabelForIframeState(frame.sessionState)}
                        </Badge>
                      </div>
                    </div>
                    <div className="settings-runtime-grid">
                      <span className="settings-runtime-row">
                        <strong>Version</strong> {frame.frameVersion}
                      </span>
                      <span className="settings-runtime-row">
                        <strong>Last used</strong> {formatTimestamp(frame.lastAccessedAt)}
                      </span>
                      <span className="settings-runtime-row">
                        <strong>Loaded at</strong> {formatTimestamp(frame.lastLoadedAt)}
                      </span>
                      <span className="settings-runtime-row settings-runtime-url">
                        <strong>URL</strong> {frame.src || "No source assigned"}
                      </span>
                      {frame.lastError ? (
                        <span className="settings-runtime-row">
                          <strong>Error</strong> {frame.lastError}
                        </span>
                      ) : null}
                    </div>
                  </article>
                ))}
              </div>
            )}
            </section>
          </TabsContent>
        </div>
      </Tabs>

      {selectedServer ? (
        <ServerDialog
          onClose={() => setSelectedServer(null)}
          onSaved={onConfigSaved}
          server={selectedServer}
        />
      ) : null}
    </div>
  );
}
