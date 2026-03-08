import { useState } from "react";
import type { ManagedServer } from "../types";
import { ServerDialog } from "./ServerDialog";

interface SettingsPageProps {
  configPath: string;
  onConfigSaved: () => void;
  servers: ManagedServer[];
}

export function SettingsPage({ configPath, onConfigSaved, servers }: SettingsPageProps) {
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

      <div className="settings-body">
        <section className="settings-section">
          <h3 className="settings-section-title">
            Configuration
          </h3>
          <div className="settings-field">
            <label className="settings-label">Config file path</label>
            <span className="settings-value settings-value-mono">{configPath}</span>
          </div>
        </section>

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
