import { useCallback, useEffect, useState } from "react";
import type { ManagedServer } from "../types";
import { loadTargetsConfig, updateTargetsConfig } from "../lib/tauri";
import { ServerDialog } from "./ServerDialog";

interface SettingsPageProps {
  configPath: string;
  onConfigSaved: () => void;
  servers: ManagedServer[];
}

export function SettingsPage({ configPath, onConfigSaved, servers }: SettingsPageProps) {
  const [configJson, setConfigJson] = useState("");
  const [originalJson, setOriginalJson] = useState("");
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);
  const [selectedServer, setSelectedServer] = useState<ManagedServer | null>(null);

  const loadConfig = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    setSaveMessage(null);

    try {
      const raw = await loadTargetsConfig();
      const parsed = JSON.parse(raw);
      const formatted = JSON.stringify(parsed, null, 2);
      setConfigJson(formatted);
      setOriginalJson(formatted);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  const handleSave = async () => {
    setIsSaving(true);
    setError(null);
    setSaveMessage(null);

    try {
      JSON.parse(configJson);
    } catch {
      setError("Invalid JSON: please check your syntax.");
      setIsSaving(false);
      return;
    }

    try {
      await updateTargetsConfig(configJson);
      setOriginalJson(configJson);
      setSaveMessage("Configuration saved. Refresh inventory to apply changes.");
      onConfigSaved();
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSaving(false);
    }
  };

  const handleRevert = () => {
    setConfigJson(originalJson);
    setError(null);
    setSaveMessage(null);
  };

  const hasChanges = configJson !== originalJson;

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

        <section className="settings-section settings-section-editor">
          <h3 className="settings-section-title">
            Configuration Editor
            {hasChanges ? (
              <span className="settings-count settings-count-modified">modified</span>
            ) : null}
          </h3>
          <p className="settings-editor-hint">
            <span className="settings-value-mono settings-editor-path">{configPath}</span>
          </p>

          {error ? <p className="settings-error">{error}</p> : null}
          {saveMessage ? <p className="settings-success">{saveMessage}</p> : null}

          <div className="settings-editor-toolbar">
            <button
              className="panel-button panel-button-toolbar"
              disabled={!hasChanges || isSaving}
              onClick={() => void handleSave()}
              type="button"
            >
              {isSaving ? "Saving…" : "Save"}
            </button>
            <button
              className="panel-button panel-button-toolbar"
              disabled={!hasChanges}
              onClick={handleRevert}
              type="button"
            >
              Revert
            </button>
            <button
              className="panel-button panel-button-toolbar"
              disabled={isLoading}
              onClick={() => void loadConfig()}
              type="button"
            >
              {isLoading ? "Loading…" : "Reload"}
            </button>
          </div>

          <textarea
            className="settings-editor-textarea"
            disabled={isLoading}
            onChange={(event) => {
              setConfigJson(event.target.value);
              setError(null);
              setSaveMessage(null);
            }}
            spellCheck={false}
            value={configJson}
          />
        </section>
      </div>

      {selectedServer ? (
        <ServerDialog
          onClose={() => setSelectedServer(null)}
          server={selectedServer}
        />
      ) : null}
    </div>
  );
}
