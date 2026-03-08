import { useCallback, useEffect, useState } from "react";
import type { ManagedServer } from "../types";
import { loadTargetsConfig, updateTargetsConfig } from "../lib/tauri";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";

type ServerDialogTab = "general" | "connection" | "containers";

interface RemoteServerConfig {
  id: string;
  label: string;
  host: string;
  description: string;
  discoveryCommand: string;
  sshUser: string;
  sshOptions: string;
  targetHostTemplate?: string;
  targetDescriptionTemplate?: string;
  notes?: string[];
}

interface ServerDialogProps {
  onClose: () => void;
  onSaved: () => void;
  server: ManagedServer;
}

export function ServerDialog({ onClose, onSaved, server }: ServerDialogProps) {
  const [activeTab, setActiveTab] = useState<ServerDialogTab>("general");
  const [form, setForm] = useState<RemoteServerConfig | null>(null);
  const [original, setOriginal] = useState<RemoteServerConfig | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);

  const loadServerConfig = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const raw = await loadTargetsConfig();
      const config = JSON.parse(raw);
      const servers: RemoteServerConfig[] = config.remoteServers ?? [];
      const match = servers.find((s) => s.id === server.id);

      if (match) {
        setForm({ ...match });
        setOriginal({ ...match });
      } else {
        setError(`Server "${server.id}" not found in config.`);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  }, [server.id]);

  useEffect(() => {
    void loadServerConfig();
  }, [loadServerConfig]);

  const updateField = (field: keyof RemoteServerConfig, value: string) => {
    setForm((prev) => (prev ? { ...prev, [field]: value } : prev));
    setError(null);
    setSaveMessage(null);
  };

  const handleSave = async () => {
    if (!form) return;

    setIsSaving(true);
    setError(null);
    setSaveMessage(null);

    try {
      const raw = await loadTargetsConfig();
      const config = JSON.parse(raw);
      const servers: RemoteServerConfig[] = config.remoteServers ?? [];
      const index = servers.findIndex((s) => s.id === server.id);

      if (index === -1) {
        setError(`Server "${server.id}" not found in config.`);
        setIsSaving(false);
        return;
      }

      // Merge form fields into the original server object (preserving surfaces etc.)
      servers[index] = { ...servers[index], ...form };
      config.remoteServers = servers;

      await updateTargetsConfig(JSON.stringify(config, null, 2));
      setOriginal({ ...form });
      setSaveMessage("Saved successfully.");
      onSaved();
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSaving(false);
    }
  };

  const hasChanges = form && original && JSON.stringify(form) !== JSON.stringify(original);

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div
        aria-label={`${server.label} server settings`}
        className="dialog-panel"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
      >
        <header className="dialog-header">
          <div className="dialog-header-copy">
            <span className="dialog-title">{server.label}</span>
            <span className="dialog-subtitle">{server.host}</span>
          </div>
          <Button
            aria-label="Close"
            className="dialog-close"
            onClick={onClose}
            size="icon"
            variant="ghost"
          >
            <svg
              aria-hidden="true"
              className="panel-button-icon-svg"
              fill="none"
              viewBox="0 0 16 16"
            >
              <path
                d="M4 4l8 8M12 4l-8 8"
                stroke="currentColor"
                strokeLinecap="round"
                strokeWidth="1.4"
              />
            </svg>
          </Button>
        </header>

        <Tabs
          onValueChange={(value) => setActiveTab(value as ServerDialogTab)}
          value={activeTab}
        >
          <TabsList className="dialog-tabs" aria-label="Server settings sections">
            {(
              [
                { id: "general", label: "General" },
                { id: "connection", label: "Connection" },
                { id: "containers", label: `Containers (${server.containers.length})` },
              ] as const
            ).map((tab) => (
              <TabsTrigger className="dialog-tab" key={tab.id} value={tab.id}>
                {tab.label}
              </TabsTrigger>
            ))}
          </TabsList>

          <div className="dialog-body">
            {isLoading ? (
              <p className="dialog-empty">Loading configuration…</p>
            ) : !form ? (
              <p className="dialog-empty">Could not load server configuration.</p>
            ) : (
              <>
                <TabsContent value="general">
                  <GeneralTab form={form} onUpdate={updateField} />
                </TabsContent>
                <TabsContent value="connection">
                  <ConnectionTab form={form} onUpdate={updateField} />
                </TabsContent>
                <TabsContent value="containers">
                  <ContainersTab server={server} />
                </TabsContent>
              </>
            )}

            {error ? <p className="dialog-error">{error}</p> : null}
            {saveMessage ? <p className="dialog-success">{saveMessage}</p> : null}
          </div>
        </Tabs>

        <footer className="dialog-footer">
          <Button
            disabled={!hasChanges || isSaving || isLoading}
            onClick={() => void handleSave()}
            variant={hasChanges ? "default" : "outline"}
          >
            {isSaving ? "Saving…" : "Save"}
          </Button>
          <Button
            onClick={onClose}
            variant="outline"
          >
            {hasChanges ? "Cancel" : "Close"}
          </Button>
        </footer>
      </div>
    </div>
  );
}

function GeneralTab({
  form,
  onUpdate,
}: {
  form: RemoteServerConfig;
  onUpdate: (field: keyof RemoteServerConfig, value: string) => void;
}) {
  return (
    <div className="dialog-tab-content">
      <EditField label="ID" value={form.id} onChange={(v) => onUpdate("id", v)} mono />
      <EditField label="Label" value={form.label} onChange={(v) => onUpdate("label", v)} />
      <EditField label="Host" value={form.host} onChange={(v) => onUpdate("host", v)} mono />
      <EditField
        label="Description"
        value={form.description}
        onChange={(v) => onUpdate("description", v)}
        multiline
      />
    </div>
  );
}

function ConnectionTab({
  form,
  onUpdate,
}: {
  form: RemoteServerConfig;
  onUpdate: (field: keyof RemoteServerConfig, value: string) => void;
}) {
  return (
    <div className="dialog-tab-content">
      <EditField
        label="SSH User"
        value={form.sshUser}
        onChange={(v) => onUpdate("sshUser", v)}
        mono
      />
      <EditField
        label="SSH Options"
        value={form.sshOptions}
        onChange={(v) => onUpdate("sshOptions", v)}
        mono
      />
      <EditField
        label="Discovery Command"
        value={form.discoveryCommand}
        onChange={(v) => onUpdate("discoveryCommand", v)}
        mono
        multiline
      />
      <EditField
        label="Host Template"
        value={form.targetHostTemplate ?? ""}
        onChange={(v) => onUpdate("targetHostTemplate", v)}
        mono
      />
      <EditField
        label="Description Template"
        value={form.targetDescriptionTemplate ?? ""}
        onChange={(v) => onUpdate("targetDescriptionTemplate", v)}
        multiline
      />
    </div>
  );
}

function ContainersTab({ server }: { server: ManagedServer }) {
  if (server.containers.length === 0) {
    return (
      <div className="dialog-tab-content">
        <p className="dialog-empty">No containers discovered on this server.</p>
      </div>
    );
  }

  return (
    <div className="dialog-tab-content">
      <table className="dialog-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Status</th>
            <th>IPv4</th>
            <th>SSH</th>
          </tr>
        </thead>
        <tbody>
          {server.containers.map((container) => (
            <tr key={container.name}>
              <td className="dialog-table-mono">{container.label}</td>
              <td>
                <Badge
                  variant={container.status === "RUNNING" ? "success" : "warning"}
                >
                  {container.status}
                </Badge>
              </td>
              <td className="dialog-table-mono">{container.ipv4 || "—"}</td>
              <td>
                <Badge
                  variant={container.sshReachable ? "success" : "destructive"}
                >
                  {container.sshState}
                </Badge>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function EditField({
  label,
  mono,
  multiline,
  onChange,
  value,
}: {
  label: string;
  mono?: boolean;
  multiline?: boolean;
  onChange: (value: string) => void;
  value: string;
}) {
  const className = `dialog-input ${mono ? "dialog-input-mono" : ""}`;

  return (
    <div className="dialog-field">
      <label className="dialog-field-label">{label}</label>
      {multiline ? (
        <textarea
          className={`${className} dialog-input-multiline`}
          onChange={(event) => onChange(event.target.value)}
          rows={3}
          spellCheck={false}
          value={value}
        />
      ) : (
        <input
          className={className}
          onChange={(event) => onChange(event.target.value)}
          spellCheck={false}
          type="text"
          value={value}
        />
      )}
    </div>
  );
}
