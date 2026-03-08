import { useEffect, useMemo, useState } from "react";
import { listBrowserFrameSnapshots } from "../lib/browser-cache";
import { restartTunnel } from "../lib/tauri";
import type {
  AppSettings,
  DeveloperTarget,
  ManagedServer,
  ThemeMode,
  TunnelStatus,
} from "../types";
import { ServerDialog } from "./ServerDialog";

type SettingsTab = "appearance" | "configuration" | "servers" | "tunnels" | "iframes";

interface SettingsPageProps {
  appSettings: AppSettings;
  configPath: string;
  onAppSettingsChange: (themeMode: ThemeMode) => void;
  onConfigSaved: () => void;
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

export function SettingsPage({
  appSettings,
  configPath,
  onAppSettingsChange,
  onConfigSaved,
  resolvedTheme,
  servers,
  targets,
  tunnelStatuses,
}: SettingsPageProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("appearance");
  const [selectedServer, setSelectedServer] = useState<ManagedServer | null>(null);
  const [restartingTunnelKey, setRestartingTunnelKey] = useState<string>("");
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
        .filter((surface) => surface.tunnelCommand.trim())
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

      <div className="settings-tab-bar" role="tablist" aria-label="Settings sections">
        {(
          [
            ["appearance", "Appearance"],
            ["configuration", "Configuration"],
            ["servers", "Servers"],
            ["tunnels", "Tunnels"],
            ["iframes", "Iframes"],
          ] as Array<[SettingsTab, string]>
        ).map(([tab, label]) => (
          <button
            aria-selected={activeTab === tab}
            className={`panel-button panel-button-toolbar settings-tab ${activeTab === tab ? "is-active" : ""}`}
            key={tab}
            onClick={() => setActiveTab(tab)}
            role="tab"
            type="button"
          >
            {label}
          </button>
        ))}
      </div>

      <div className="settings-body">
        {activeTab === "appearance" ? (
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
                  <button
                    aria-checked={appSettings.themeMode === themeMode}
                    className={`panel-button panel-button-toolbar settings-option-button ${appSettings.themeMode === themeMode ? "is-active" : ""}`}
                    key={themeMode}
                    onClick={() => onAppSettingsChange(themeMode)}
                    role="radio"
                    type="button"
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>
            <div className="settings-field">
              <label className="settings-label">Resolved theme</label>
              <span className="settings-value">{resolvedTheme}</span>
            </div>
          </section>
        ) : null}

        {activeTab === "configuration" ? (
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
        ) : null}

        {activeTab === "servers" ? (
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
                      <span
                        className={`settings-server-state settings-server-state-${server.state}`}
                      >
                        {server.state}
                      </span>
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
        ) : null}

        {activeTab === "tunnels" ? (
          <section className="settings-section">
            <h3 className="settings-section-title">
              Tunnel sessions
              <span className="settings-count">{tunnelEntries.length}</span>
            </h3>
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
                        <span
                          className={`settings-server-state settings-server-state-${status?.state ?? "stopped"}`}
                        >
                          {status?.state ?? "stopped"}
                        </span>
                        <button
                          className="panel-button panel-button-toolbar"
                          disabled={restartingTunnelKey === cacheKey}
                          onClick={() => void restartTunnelEntry(target.id, surface.id, cacheKey)}
                          type="button"
                        >
                          {restartingTunnelKey === cacheKey ? "Restarting…" : "Restart"}
                        </button>
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
        ) : null}

        {activeTab === "iframes" ? (
          <section className="settings-section">
            <h3 className="settings-section-title">
              Iframe cache
              <span className="settings-count">{iframeSnapshots.length}</span>
            </h3>
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
                      <span
                        className={`settings-server-state settings-server-state-${frame.hasOwner ? "running" : "pending"}`}
                      >
                        {frame.hasOwner ? "attached" : "parked"}
                      </span>
                    </div>
                    <div className="settings-runtime-grid">
                      <span className="settings-runtime-row">
                        <strong>Version</strong> {frame.frameVersion}
                      </span>
                      <span className="settings-runtime-row">
                        <strong>Last used</strong> {formatTimestamp(frame.lastAccessedAt)}
                      </span>
                      <span className="settings-runtime-row settings-runtime-url">
                        <strong>URL</strong> {frame.src || "No source assigned"}
                      </span>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>
        ) : null}
      </div>

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
