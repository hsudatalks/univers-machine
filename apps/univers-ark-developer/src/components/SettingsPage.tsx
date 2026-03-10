import { useState } from "react";
import type {
  AppSettings,
  ManagedServer,
  ThemeMode,
} from "../types";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { ServerDialog } from "./ServerDialog";

type SettingsTab = "appearance" | "configuration" | "servers";

interface SettingsPageProps {
  appSettings: AppSettings;
  configPath: string;
  onDashboardRefreshChange: (seconds: number) => void;
  onConfigSaved: () => void;
  onThemeModeChange: (themeMode: ThemeMode) => void;
  resolvedTheme: "light" | "dark";
  servers: ManagedServer[];
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

export function SettingsPage({
  appSettings,
  configPath,
  onDashboardRefreshChange,
  onConfigSaved,
  onThemeModeChange,
  resolvedTheme,
  servers,
}: SettingsPageProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("appearance");
  const [selectedServer, setSelectedServer] = useState<ManagedServer | null>(null);

  return (
    <div className="settings-page">
      <header className="panel-header settings-header">
        <div className="settings-header-copy">
          <span className="panel-title">Settings</span>
          <p className="panel-description settings-description">
            Application configuration and server inventory
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
              <label className="settings-label">Container count</label>
              <span className="settings-value">
                {servers.reduce((sum, server) => sum + server.containers.length, 0)} container(s)
              </span>
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
