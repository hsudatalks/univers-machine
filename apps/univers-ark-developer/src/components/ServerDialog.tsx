import { useCallback, useEffect, useMemo, useState } from "react";
import type { ManagedServer } from "../types";
import { loadTargetsConfig, updateTargetsConfig } from "../lib/tauri";
import {
  createEmptyManualContainer,
  createEmptyServer,
  parseTargetsConfig,
  stringifyTargetsConfig,
  type ContainerDiscoveryMode,
  type ContainerManagerType,
  type RemoteServerConfig,
  type TargetsConfigDocument,
} from "../lib/targets-config";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";

type ServerDialogTab = "general" | "connection" | "discovery" | "containers";

interface ServerDialogProps {
  onClose: () => void;
  onSaved: () => void;
  server?: ManagedServer | null;
  defaultProfileId?: string;
}

export function ServerDialog({
  onClose,
  onSaved,
  server,
  defaultProfileId = "",
}: ServerDialogProps) {
  const [activeTab, setActiveTab] = useState<ServerDialogTab>("general");
  const [config, setConfig] = useState<TargetsConfigDocument | null>(null);
  const [form, setForm] = useState<RemoteServerConfig>(createEmptyServer(defaultProfileId));
  const [originalId, setOriginalId] = useState<string | null>(server?.id ?? null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);

  const isCreateMode = !server;

  const loadServerConfig = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const raw = await loadTargetsConfig();
      const parsed = parseTargetsConfig(raw);
      setConfig(parsed);

      if (server) {
        const match = parsed.remoteServers.find((entry) => entry.id === server.id);
        if (match) {
          setForm(match);
          setOriginalId(match.id);
        } else {
          setError(`Server "${server.id}" not found in config.`);
        }
      } else {
        setForm(createEmptyServer(defaultProfileId || Object.keys(parsed.profiles)[0] || ""));
      }
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setIsLoading(false);
    }
  }, [defaultProfileId, server]);

  useEffect(() => {
    void loadServerConfig();
  }, [loadServerConfig]);

  const profileOptions = useMemo(
    () => Object.keys(config?.profiles ?? {}).sort(),
    [config],
  );

  const hasIdConflict = useMemo(() => {
    if (!config) return false;
    const nextId = form.id.trim();
    if (!nextId) return false;
    return config.remoteServers.some((entry) => entry.id === nextId && entry.id !== originalId);
  }, [config, form.id, originalId]);

  const updateField = <K extends keyof RemoteServerConfig>(field: K, value: RemoteServerConfig[K]) => {
    setForm((prev) => ({ ...prev, [field]: value }));
    setError(null);
    setSaveMessage(null);
  };

  const updateWorkspaceField = (field: keyof RemoteServerConfig["workspace"], value: string) => {
    setForm((prev) => ({
      ...prev,
      workspace: {
        ...prev.workspace,
        [field]: value,
      },
    }));
    setError(null);
    setSaveMessage(null);
  };

  const updateManualContainerField = (
    index: number,
    field: keyof RemoteServerConfig["manualContainers"][number],
    value: string,
  ) => {
    setForm((prev) => ({
      ...prev,
      manualContainers: prev.manualContainers.map((container, containerIndex) =>
        containerIndex === index ? { ...container, [field]: value } : container,
      ),
    }));
    setError(null);
    setSaveMessage(null);
  };

  const handleSave = async () => {
    if (!config) return;

    const nextId = form.id.trim();
    if (!nextId) {
      setError("Server ID is required.");
      return;
    }
    if (hasIdConflict) {
      setError(`Server "${nextId}" already exists.`);
      return;
    }

    setIsSaving(true);
    setError(null);
    setSaveMessage(null);

    try {
      const nextConfig: TargetsConfigDocument = {
        ...config,
        remoteServers: [...config.remoteServers],
      };
      const nextServer: RemoteServerConfig = {
        ...form,
        id: nextId,
        manualContainers:
          form.discoveryMode === "manual"
            ? form.manualContainers.filter(
                (container) => container.name.trim() && container.ipv4.trim(),
              )
            : [],
      };

      const existingIndex = nextConfig.remoteServers.findIndex(
        (entry) => entry.id === originalId,
      );

      if (existingIndex >= 0) {
        nextConfig.remoteServers[existingIndex] = nextServer;
      } else {
        nextConfig.remoteServers.push(nextServer);
      }

      await updateTargetsConfig(stringifyTargetsConfig(nextConfig));
      setConfig(nextConfig);
      setOriginalId(nextServer.id);
      setForm(nextServer);
      setSaveMessage("Saved successfully.");
      onSaved();
    } catch (saveError) {
      setError(String(saveError));
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div
        aria-label={isCreateMode ? "Create server" : `${form.label || form.id} server settings`}
        className="dialog-panel"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
      >
        <header className="dialog-header">
          <div className="dialog-header-copy">
            <span className="dialog-title">{isCreateMode ? "New server" : form.label || form.id}</span>
            <span className="dialog-subtitle">{form.host || "Define connection and discovery settings"}</span>
          </div>
          <Button
            aria-label="Close"
            className="dialog-close"
            onClick={onClose}
            size="icon"
            variant="ghost"
          >
            <svg aria-hidden="true" className="panel-button-icon-svg" fill="none" viewBox="0 0 16 16">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeLinecap="round" strokeWidth="1.4" />
            </svg>
          </Button>
        </header>

        <Tabs onValueChange={(value) => setActiveTab(value as ServerDialogTab)} value={activeTab}>
          <TabsList className="dialog-tabs" aria-label="Server settings sections">
            <TabsTrigger className="dialog-tab" value="general">General</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="connection">Connection</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="discovery">Discovery</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="containers">
              Containers
              {form.discoveryMode === "manual" ? ` (${form.manualContainers.length})` : server ? ` (${server.containers.length})` : ""}
            </TabsTrigger>
          </TabsList>

          <div className="dialog-body">
            {isLoading ? (
              <p className="dialog-empty">Loading configuration…</p>
            ) : (
              <>
                <TabsContent value="general">
                  <div className="dialog-tab-content">
                    <EditField label="ID" mono onChange={(value) => updateField("id", value)} value={form.id} />
                    <EditField label="Label" onChange={(value) => updateField("label", value)} value={form.label} />
                    <EditField label="Host" mono onChange={(value) => updateField("host", value)} value={form.host} />
                    <EditField
                      label="Description"
                      multiline
                      onChange={(value) => updateField("description", value)}
                      value={form.description}
                    />
                    <SelectField
                      label="Profile"
                      onChange={(value) => updateWorkspaceField("profile", value)}
                      options={profileOptions.map((profile) => ({ value: profile, label: profile }))}
                      value={form.workspace.profile}
                    />
                  </div>
                </TabsContent>

                <TabsContent value="connection">
                  <div className="dialog-tab-content">
                    <EditField
                      label="SSH User"
                      mono
                      onChange={(value) => updateField("sshUser", value)}
                      value={form.sshUser}
                    />
                    <EditField
                      label="SSH Options"
                      mono
                      onChange={(value) => updateField("sshOptions", value)}
                      value={form.sshOptions}
                    />
                    <EditField
                      label="Terminal Command Template"
                      multiline
                      mono
                      onChange={(value) => updateField("terminalCommandTemplate", value)}
                      value={form.terminalCommandTemplate}
                    />
                  </div>
                </TabsContent>

                <TabsContent value="discovery">
                  <div className="dialog-tab-content">
                    <SelectField
                      label="Container manager"
                      onChange={(value) => updateField("managerType", value as ContainerManagerType)}
                      options={[
                        { value: "lxd", label: "LXD" },
                        { value: "docker", label: "Docker" },
                        { value: "orbstack", label: "OrbStack" },
                      ]}
                      value={form.managerType}
                    />
                    <SelectField
                      label="Discovery mode"
                      onChange={(value) => updateField("discoveryMode", value as ContainerDiscoveryMode)}
                      options={[
                        { value: "auto", label: "Auto scan" },
                        { value: "manual", label: "Manual containers" },
                      ]}
                      value={form.discoveryMode}
                    />
                    <EditField
                      label="Discovery Command Override"
                      mono
                      multiline
                      onChange={(value) => updateField("discoveryCommand", value)}
                      value={form.discoveryCommand}
                    />
                    <EditField
                      label="Container Name Suffix"
                      mono
                      onChange={(value) => updateField("containerNameSuffix", value)}
                      value={form.containerNameSuffix}
                    />
                    <EditField
                      label="Target Label Template"
                      onChange={(value) => updateField("targetLabelTemplate", value)}
                      value={form.targetLabelTemplate}
                    />
                    <EditField
                      label="Target Host Template"
                      mono
                      onChange={(value) => updateField("targetHostTemplate", value)}
                      value={form.targetHostTemplate}
                    />
                    <EditField
                      label="Target Description Template"
                      multiline
                      onChange={(value) => updateField("targetDescriptionTemplate", value)}
                      value={form.targetDescriptionTemplate}
                    />
                  </div>
                </TabsContent>

                <TabsContent value="containers">
                  {form.discoveryMode === "manual" ? (
                    <div className="dialog-tab-content">
                      <div className="dialog-section-actions">
                        <Button onClick={() => updateField("manualContainers", [...form.manualContainers, createEmptyManualContainer()])} size="sm" variant="outline">
                          Add container
                        </Button>
                      </div>
                      {form.manualContainers.length === 0 ? (
                        <p className="dialog-empty">No manual containers defined.</p>
                      ) : (
                        <div className="dialog-list">
                          {form.manualContainers.map((container, index) => (
                            <div className="dialog-card" key={`${container.name || "container"}-${index}`}>
                              <div className="dialog-card-header">
                                <span className="dialog-card-title">{container.label || container.name || `Container ${index + 1}`}</span>
                                <Button
                                  onClick={() =>
                                    updateField(
                                      "manualContainers",
                                      form.manualContainers.filter((_, itemIndex) => itemIndex !== index),
                                    )
                                  }
                                  size="sm"
                                  variant="ghost"
                                >
                                  Remove
                                </Button>
                              </div>
                              <div className="dialog-field-grid">
                                <EditField label="Name" mono onChange={(value) => updateManualContainerField(index, "name", value)} value={container.name} />
                                <EditField label="Label" onChange={(value) => updateManualContainerField(index, "label", value)} value={container.label} />
                                <EditField label="IPv4" mono onChange={(value) => updateManualContainerField(index, "ipv4", value)} value={container.ipv4} />
                                <EditField label="Status" mono onChange={(value) => updateManualContainerField(index, "status", value)} value={container.status} />
                              </div>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  ) : server ? (
                    <ContainersTable server={server} />
                  ) : (
                    <div className="dialog-tab-content">
                      <p className="dialog-empty">Save the server first, then refresh inventory to view discovered containers.</p>
                    </div>
                  )}
                </TabsContent>
              </>
            )}
            {error ? <p className="dialog-error">{error}</p> : null}
            {saveMessage ? <p className="dialog-success">{saveMessage}</p> : null}
          </div>
        </Tabs>

        <footer className="dialog-footer">
          <Button disabled={isLoading || isSaving} onClick={() => void handleSave()}>
            {isSaving ? "Saving…" : "Save"}
          </Button>
          <Button onClick={onClose} variant="outline">
            Close
          </Button>
        </footer>
      </div>
    </div>
  );
}

function ContainersTable({ server }: { server: ManagedServer }) {
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
                <Badge variant={container.status === "RUNNING" ? "success" : "warning"}>
                  {container.status}
                </Badge>
              </td>
              <td className="dialog-table-mono">{container.ipv4 || "—"}</td>
              <td>
                <Badge variant={container.sshReachable ? "success" : "destructive"}>
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

function SelectField({
  label,
  onChange,
  options,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  options: Array<{ value: string; label: string }>;
  value: string;
}) {
  return (
    <div className="dialog-field">
      <label className="dialog-field-label">{label}</label>
      <select className="dialog-input" onChange={(event) => onChange(event.target.value)} value={value}>
        <option value="">Select…</option>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
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
