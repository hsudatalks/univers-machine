import { useEffect, useState } from "react";
import type {
  AppDiagnostics,
  AppSettings,
  ManagedMachine,
  ThemeMode,
} from "../types";
import { visibleContainers } from "../lib/container-visibility";
import { listBrowserFrameSnapshots } from "../lib/browser-cache";
import { loadAppDiagnostics, loadTargetsConfig, updateTargetsConfig } from "../lib/tauri";
import { parseTargetsConfig, stringifyTargetsConfig } from "../lib/targets-config";
import { ConnectionStatusLight } from "./ConnectionStatusLight";
import { ProfileDialog } from "./ProfileDialog";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { ServerDialog } from "./ServerDialog";

type SettingsTab = "appearance" | "configuration" | "profiles" | "machines" | "diagnostics";

interface SettingsPageProps {
  appSettings: AppSettings;
  configPath: string;
  onAddMachine: () => void;
  onDashboardRefreshChange: (seconds: number) => void;
  onConfigSaved: () => void;
  onThemeModeChange: (themeMode: ThemeMode) => void;
  resolvedTheme: "light" | "dark";
  machines: ManagedMachine[];
}

export function SettingsPage({
  appSettings,
  configPath,
  onAddMachine,
  onDashboardRefreshChange,
  onConfigSaved,
  onThemeModeChange,
  resolvedTheme,
  machines,
}: SettingsPageProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("appearance");
  const [selectedMachine, setSelectedMachine] = useState<ManagedMachine | null>(null);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [isCreatingProfile, setIsCreatingProfile] = useState(false);
  const [profileIds, setProfileIds] = useState<string[]>([]);
  const [defaultProfileId, setDefaultProfileId] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<AppDiagnostics | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [diagnosticsRefreshing, setDiagnosticsRefreshing] = useState(false);

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

  useEffect(() => {
    if (activeTab !== "diagnostics") {
      return;
    }

    let cancelled = false;
    let intervalId: number | undefined;

    const refreshDiagnostics = (showBusy = false) => {
      if (showBusy) {
        setDiagnosticsRefreshing(true);
      }
      void loadAppDiagnostics()
        .then((snapshot) => {
          if (cancelled) {
            return;
          }
          setDiagnostics(snapshot);
          setDiagnosticsError(null);
        })
        .catch((error) => {
          if (cancelled) {
            return;
          }
          setDiagnosticsError(error instanceof Error ? error.message : String(error));
        })
        .finally(() => {
          if (!cancelled) {
            setDiagnosticsRefreshing(false);
          }
        });
    };

    refreshDiagnostics(true);
    intervalId = window.setInterval(() => refreshDiagnostics(false), 2000);

    return () => {
      cancelled = true;
      if (intervalId) {
        window.clearInterval(intervalId);
      }
    };
  }, [activeTab]);

  const formatCounts = (counts: Record<string, number>) => {
    const entries = Object.entries(counts).sort(([left], [right]) => left.localeCompare(right));
    if (entries.length === 0) {
      return "none";
    }

    return entries.map(([key, count]) => `${key}:${count}`).join(" · ");
  };

  const formatTimestamp = (timestampMs: number) => {
    if (!timestampMs) {
      return "none";
    }

    return new Date(timestampMs).toLocaleString();
  };

  const formatDurationMs = (durationMs: number) => {
    if (durationMs <= 0) {
      return "now";
    }
    if (durationMs < 1000) {
      return `${durationMs}ms`;
    }
    const seconds = durationMs / 1000;
    if (seconds < 60) {
      return `${seconds.toFixed(seconds >= 10 ? 0 : 1)}s`;
    }
    const minutes = seconds / 60;
    return `${minutes.toFixed(minutes >= 10 ? 0 : 1)}m`;
  };

  const browserFrames = listBrowserFrameSnapshots();
  const browserFrameStateCounts = browserFrames.reduce<Record<string, number>>((counts, frame) => {
    counts[frame.sessionState] = (counts[frame.sessionState] ?? 0) + 1;
    return counts;
  }, {});
  const ownedBrowserFrameCount = browserFrames.filter((frame) => frame.hasOwner).length;
  const prerenderedBrowserFrameCount = browserFrames.length - ownedBrowserFrameCount;

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
              ["diagnostics", "Diagnostics"],
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
                {machines.reduce((sum, machine) => sum + visibleContainers(machine.containers).length, 0)} container(s)
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
              <Button onClick={onAddMachine} size="sm" variant="outline">
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
                        <ConnectionStatusLight state={machine.state} />
                      </div>
                      <div className="settings-server-meta">
                        <span className="settings-server-host">{machine.host}</span>
                      {machine.description ? (
                        <span className="settings-server-desc">{machine.description}</span>
                      ) : null}
                    </div>
                    <div className="settings-server-footer">
                      <span className="settings-server-containers">
                        {visibleContainers(machine.containers).length} container(s)
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

          <TabsContent className="settings-tab-panel" value="diagnostics">
            <section className="settings-section">
              <div className="settings-section-heading">
                <h3 className="settings-section-title">Diagnostics</h3>
                <Button
                  onClick={() => {
                    setDiagnosticsRefreshing(true);
                    void loadAppDiagnostics()
                      .then((snapshot) => {
                        setDiagnostics(snapshot);
                        setDiagnosticsError(null);
                      })
                      .catch((error) => {
                        setDiagnosticsError(error instanceof Error ? error.message : String(error));
                      })
                      .finally(() => setDiagnosticsRefreshing(false));
                  }}
                  size="sm"
                  variant="outline"
                >
                  {diagnosticsRefreshing ? "Refreshing…" : "Refresh"}
                </Button>
              </div>
              <p className="settings-editor-hint">
                Process-local runtime diagnostics. Dev and prod already use separate config files and
                port ranges; cross-process coordination is mainly about shared resource pressure, not
                state corruption.
              </p>
              {diagnosticsError ? (
                <p className="settings-empty">{diagnosticsError}</p>
              ) : diagnostics ? (
                <div className="settings-diagnostics-grid">
                  <section className="settings-runtime-card">
                    <div className="settings-runtime-header">
                      <div className="settings-runtime-copy">
                        <span className="settings-runtime-label">Process</span>
                        <span className="settings-runtime-key">
                          {diagnostics.channel} · pid {diagnostics.processId}
                        </span>
                      </div>
                    </div>
                    <div className="settings-runtime-grid">
                      <div className="settings-runtime-row">
                        <strong>Config</strong>
                        <span className="settings-runtime-url">{diagnostics.configPath}</span>
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Surface ports</strong>
                        {diagnostics.surfacePorts.start}-{diagnostics.surfacePorts.end}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Tunnel ports</strong>
                        {diagnostics.internalTunnelPorts.start}-{diagnostics.internalTunnelPorts.end}
                      </div>
                    </div>
                  </section>

                  <section className="settings-runtime-card">
                    <div className="settings-runtime-header">
                      <div className="settings-runtime-copy">
                        <span className="settings-runtime-label">Activity</span>
                        <span className="settings-runtime-key">frontend hint -&gt; scheduler</span>
                      </div>
                    </div>
                    <div className="settings-runtime-grid">
                      <div className="settings-runtime-row">
                        <strong>Visible</strong>
                        {diagnostics.activity.visible ? "yes" : "no"} · <strong>Focused</strong>{" "}
                        {diagnostics.activity.focused ? "yes" : "no"} · <strong>Online</strong>{" "}
                        {diagnostics.activity.online ? "yes" : "no"}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Recovering</strong>
                        {diagnostics.activity.recovering ? "yes" : "no"}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Recovery generation</strong>
                        {diagnostics.activity.recoveryGeneration}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Last recovery</strong>
                        {formatTimestamp(diagnostics.activity.lastRecoveryStartedAtMs)}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Active machine</strong>
                        {diagnostics.activity.activeMachineId ?? "none"}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Active target</strong>
                        {diagnostics.activity.activeTargetId ?? "none"}
                      </div>
                    </div>
                  </section>

                  <section className="settings-runtime-card">
                    <div className="settings-runtime-header">
                      <div className="settings-runtime-copy">
                        <span className="settings-runtime-label">Scheduler Budget</span>
                        <span className="settings-runtime-key">per scheduler cycle</span>
                      </div>
                    </div>
                    <div className="settings-runtime-grid">
                      <div className="settings-runtime-row">
                        <strong>Tunnel reconciles</strong>
                        {diagnostics.scheduler.maxTunnelReconciles}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Connectivity probes</strong>
                        {diagnostics.scheduler.maxConnectivityProbes}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Dashboard refreshes</strong>
                        {diagnostics.scheduler.maxDashboardRefreshes}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Next wake</strong>
                        {formatDurationMs(diagnostics.scheduler.nextWakeInMs)}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Last cycle start</strong>
                        {formatTimestamp(diagnostics.scheduler.lastCycleStartedAtMs)}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Last cycle end</strong>
                        {formatTimestamp(diagnostics.scheduler.lastCycleFinishedAtMs)}
                      </div>
                    </div>
                  </section>

                  <section className="settings-runtime-card">
                    <div className="settings-runtime-header">
                      <div className="settings-runtime-copy">
                        <span className="settings-runtime-label">Terminals</span>
                        <span className="settings-runtime-key">active russh terminal sessions</span>
                      </div>
                    </div>
                    <div className="settings-runtime-grid">
                      <div className="settings-runtime-row">
                        <strong>Sessions</strong>
                        {diagnostics.terminals.sessionCount}
                      </div>
                    </div>
                  </section>

                  <section className="settings-runtime-card">
                    <div className="settings-runtime-header">
                      <div className="settings-runtime-copy">
                        <span className="settings-runtime-label">Browser Cache</span>
                        <span className="settings-runtime-key">hidden iframe / prerender pool</span>
                      </div>
                    </div>
                    <div className="settings-runtime-grid">
                      <div className="settings-runtime-row">
                        <strong>Total frames</strong>
                        {browserFrames.length} · <strong>Owned</strong> {ownedBrowserFrameCount} ·{" "}
                        <strong>Parked</strong> {prerenderedBrowserFrameCount}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>States</strong>
                        {formatCounts(browserFrameStateCounts)}
                      </div>
                    </div>
                  </section>

                  <section className="settings-runtime-card">
                    <div className="settings-runtime-header">
                      <div className="settings-runtime-copy">
                        <span className="settings-runtime-label">Tunnels</span>
                        <span className="settings-runtime-key">session + desired state</span>
                      </div>
                    </div>
                    <div className="settings-runtime-grid">
                      <div className="settings-runtime-row">
                        <strong>Desired</strong>
                        {diagnostics.tunnels.desiredCount} · <strong>Sessions</strong>{" "}
                        {diagnostics.tunnels.sessionCount} · <strong>Ready</strong>{" "}
                        {diagnostics.tunnels.readySessionCount}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Local ports</strong>
                        {diagnostics.tunnels.localPortCount}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Event rate</strong>
                        {diagnostics.tunnels.statusEventsPerMinute} batch/min · <strong>Items</strong>{" "}
                        {diagnostics.tunnels.statusItemsPerMinute}/min · <strong>Reconciles</strong>{" "}
                        {diagnostics.tunnels.reconcilesPerMinute}/min
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Next due</strong>
                        {formatDurationMs(diagnostics.tunnels.nextDueInMs)} · <strong>Due now</strong>{" "}
                        {diagnostics.tunnels.dueNowCount} · <strong>Waiting</strong>{" "}
                        {diagnostics.tunnels.waitingCount}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>States</strong>
                        {formatCounts(diagnostics.tunnels.statusCounts)}
                      </div>
                    </div>
                  </section>

                  <section className="settings-runtime-card">
                    <div className="settings-runtime-header">
                      <div className="settings-runtime-copy">
                        <span className="settings-runtime-label">Connectivity</span>
                        <span className="settings-runtime-key">machine + container snapshots</span>
                      </div>
                    </div>
                    <div className="settings-runtime-grid">
                      <div className="settings-runtime-row">
                        <strong>Machines</strong>
                        {diagnostics.connectivity.machineSnapshotCount} · <strong>Containers</strong>{" "}
                        {diagnostics.connectivity.containerSnapshotCount}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Event rate</strong>
                        {diagnostics.connectivity.statusEventsPerMinute} batch/min · <strong>Items</strong>{" "}
                        {diagnostics.connectivity.statusItemsPerMinute}/min
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Probe rate</strong>
                        {diagnostics.connectivity.probesPerMinute}/min · <strong>Next due</strong>{" "}
                        {formatDurationMs(diagnostics.connectivity.nextDueInMs)}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Due now</strong>
                        {diagnostics.connectivity.dueNowCount} · <strong>Backoff targets</strong>{" "}
                        {diagnostics.connectivity.backoffTargetCount} · <strong>Max failures</strong>{" "}
                        {diagnostics.connectivity.maxConsecutiveFailures}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Machine states</strong>
                        {formatCounts(diagnostics.connectivity.machineStateCounts)}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Container states</strong>
                        {formatCounts(diagnostics.connectivity.containerStateCounts)}
                      </div>
                    </div>
                  </section>

                  <section className="settings-runtime-card">
                    <div className="settings-runtime-header">
                      <div className="settings-runtime-copy">
                        <span className="settings-runtime-label">Dashboards</span>
                        <span className="settings-runtime-key">registered background refreshes</span>
                      </div>
                    </div>
                    <div className="settings-runtime-grid">
                      <div className="settings-runtime-row">
                        <strong>Registered</strong>
                        {diagnostics.dashboards.registeredCount}
                      </div>
                      <div className="settings-runtime-row">
                        <strong>Update rate</strong>
                        {diagnostics.dashboards.updatesPerMinute}/min · <strong>Next due</strong>{" "}
                        {formatDurationMs(diagnostics.dashboards.nextDueInMs)} · <strong>Due now</strong>{" "}
                        {diagnostics.dashboards.dueNowCount}
                      </div>
                    </div>
                  </section>
                </div>
              ) : (
                <p className="settings-empty">Loading diagnostics…</p>
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
