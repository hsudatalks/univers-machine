import { useEffect, useState } from "react";
import type {
  AppSettings,
  ManagedMachine,
  ThemeMode,
} from "../types";
import { loadTargetsConfig, updateTargetsConfig } from "../lib/tauri";
import { parseTargetsConfig, stringifyTargetsConfig } from "../lib/targets-config";
import { ProfileDialog } from "./ProfileDialog";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { ServerDialog } from "./ServerDialog";

type SettingsTab = "appearance" | "configuration" | "profiles" | "machines";

interface SettingsPageProps {
  appSettings: AppSettings;
  configPath: string;
  onDashboardRefreshChange: (seconds: number) => void;
  onConfigSaved: () => void;
  onThemeModeChange: (themeMode: ThemeMode) => void;
  resolvedTheme: "light" | "dark";
  machines: ManagedMachine[];
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
  machines,
}: SettingsPageProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("appearance");
  const [selectedMachine, setSelectedMachine] = useState<ManagedMachine | null>(null);
  const [isCreatingMachine, setIsCreatingMachine] = useState(false);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [isCreatingProfile, setIsCreatingProfile] = useState(false);
  const [profileIds, setProfileIds] = useState<string[]>([]);
  const [defaultProfileId, setDefaultProfileId] = useState<string | null>(null);

  const refreshProfiles = () => {
    void loadTargetsConfig()
      .then((raw) => {
        const parsed = parseTargetsConfig(raw);
        setProfileIds(Object.keys(parsed.profiles).sort());
        setDefaultProfileId(parsed.defaultProfile ?? null);
      })
      .catch(() => {
        setProfileIds([]);
        setDefaultProfileId(null);
      });
  };

  const updateDefaultProfile = (profileId: string | null) => {
    void loadTargetsConfig()
      .then(async (raw) => {
        const parsed = parseTargetsConfig(raw);
        parsed.defaultProfile = profileId;
        await updateTargetsConfig(stringifyTargetsConfig(parsed));
        setDefaultProfileId(profileId);
        onConfigSaved();
      })
      .catch(() => {});
  };

  useEffect(() => {
    refreshProfiles();
  }, [configPath]);

  return (
    <div className="settings-page">
      <header className="panel-header settings-header">
        <div className="settings-header-copy">
          <span className="panel-title">Settings</span>
          <p className="panel-description settings-description">
            Application configuration and machine inventory
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
              ["profiles", "Profiles"],
              ["machines", "Machines"],
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
                {machines.reduce((sum, machine) => sum + machine.containers.length, 0)} container(s)
              </span>
            </div>
            </section>
          </TabsContent>

          <TabsContent className="settings-tab-panel" value="machines">
            <section className="settings-section">
            <div className="settings-section-heading">
              <h3 className="settings-section-title">
                Machines
                <span className="settings-count">{machines.length}</span>
              </h3>
              <Button onClick={() => setIsCreatingMachine(true)} size="sm" variant="outline">
                Add machine
              </Button>
            </div>
            {machines.length === 0 ? (
              <p className="settings-empty">No machines configured.</p>
            ) : (
              <div className="settings-server-list">
                {machines.map((machine) => (
                  <button
                    className="settings-server-card"
                    key={machine.id}
                    onClick={() => setSelectedMachine(machine)}
                    type="button"
                  >
                    <div className="settings-server-header">
                      <span className="settings-server-label">{machine.label}</span>
                      <Badge variant={badgeVariantForState(machine.state)}>
                        {machine.state}
                      </Badge>
                    </div>
                    <div className="settings-server-meta">
                      <span className="settings-server-host">{machine.host}</span>
                      {machine.description ? (
                        <span className="settings-server-desc">{machine.description}</span>
                      ) : null}
                    </div>
                    <div className="settings-server-footer">
                      <span className="settings-server-containers">
                        {machine.containers.length} container(s)
                      </span>
                      {machine.message ? (
                        <span className="settings-server-message">{machine.message}</span>
                      ) : null}
                    </div>
                  </button>
                ))}
              </div>
            )}
            </section>
          </TabsContent>

          <TabsContent className="settings-tab-panel" value="profiles">
            <section className="settings-section">
              <div className="settings-section-heading">
                <h3 className="settings-section-title">
                  Profiles
                  <span className="settings-count">{profileIds.length}</span>
                </h3>
                <Button onClick={() => setIsCreatingProfile(true)} size="sm" variant="outline">
                  Add profile
                </Button>
              </div>
              {profileIds.length === 0 ? (
                <p className="settings-empty">No profiles configured.</p>
              ) : (
                <div className="settings-server-list">
                  <div className="settings-field">
                    <label className="settings-label">Default profile</label>
                    <div className="settings-option-group">
                      <span className="settings-value settings-value-mono">
                        {defaultProfileId || "None"}
                      </span>
                      {defaultProfileId ? (
                        <Button
                          onClick={() => updateDefaultProfile(null)}
                          size="sm"
                          variant="outline"
                        >
                          Clear
                        </Button>
                      ) : null}
                    </div>
                  </div>
                  {profileIds.map((profileId) => (
                    <button
                      className="settings-server-card"
                      key={profileId}
                      onClick={() => setSelectedProfileId(profileId)}
                      type="button"
                    >
                      <div className="settings-server-header">
                        <span className="settings-server-label">{profileId}</span>
                        <div className="settings-option-group">
                          {defaultProfileId === profileId ? (
                            <Badge variant="success">Default</Badge>
                          ) : (
                            <Button
                              onClick={(event) => {
                                event.stopPropagation();
                                updateDefaultProfile(profileId);
                              }}
                              size="sm"
                              variant="outline"
                            >
                              Set default
                            </Button>
                          )}
                          <Badge variant="neutral">Profile</Badge>
                        </div>
                      </div>
                      <div className="settings-server-meta">
                        <span className="settings-server-host">Workspace and services defaults</span>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </section>
          </TabsContent>
        </div>
      </Tabs>

      {selectedMachine ? (
        <ServerDialog
          onClose={() => setSelectedMachine(null)}
          onSaved={() => {
            refreshProfiles();
            onConfigSaved();
          }}
          server={selectedMachine}
        />
      ) : null}
      {isCreatingMachine ? (
        <ServerDialog
          defaultProfileId={defaultProfileId ?? profileIds[0] ?? ""}
          onClose={() => setIsCreatingMachine(false)}
          onSaved={() => {
            setIsCreatingMachine(false);
            refreshProfiles();
            onConfigSaved();
          }}
        />
      ) : null}
      {selectedProfileId ? (
        <ProfileDialog
          onClose={() => setSelectedProfileId(null)}
          onSaved={() => {
            refreshProfiles();
            onConfigSaved();
          }}
          profileId={selectedProfileId}
        />
      ) : null}
      {isCreatingProfile ? (
        <ProfileDialog
          onClose={() => setIsCreatingProfile(false)}
          onSaved={() => {
            setIsCreatingProfile(false);
            refreshProfiles();
            onConfigSaved();
          }}
        />
      ) : null}
    </div>
  );
}
