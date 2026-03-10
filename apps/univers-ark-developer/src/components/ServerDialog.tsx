import { useCallback, useEffect, useMemo, useState } from "react";
import type { ManagedMachine } from "../types";
import { loadTargetsConfig, scanMachineInventory, updateTargetsConfig } from "../lib/tauri";
import {
  createEmptyMachine,
  createEmptyMachineContainer,
  parseTargetsConfig,
  stringifyTargetsConfig,
  type ContainerDiscoveryMode,
  type MachineConfig,
  type TargetsConfigDocument,
} from "../lib/targets-config";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";

type ServerDialogTab = "general" | "connection" | "discovery" | "containers";

interface ServerDialogProps {
  onClose: () => void;
  onSaved: () => void;
  server?: ManagedMachine | null;
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
  const [form, setForm] = useState<MachineConfig>(createEmptyMachine(defaultProfileId));
  const [originalId, setOriginalId] = useState<string | null>(server?.id ?? null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);
  const [isScanning, setIsScanning] = useState(false);

  const isCreateMode = !server;

  const loadMachineFromDisk = useCallback(
    async (machineId?: string | null) => {
      const raw = await loadTargetsConfig();
      const parsed = parseTargetsConfig(raw);
      setConfig(parsed);

      if (machineId) {
        const match = parsed.machines.find((entry) => entry.id === machineId);
        if (!match) {
          throw new Error(`Machine "${machineId}" not found in config.`);
        }

        setForm(match);
        setOriginalId(match.id);
        return;
      }

      setForm(createEmptyMachine(defaultProfileId || Object.keys(parsed.profiles)[0] || ""));
    },
    [defaultProfileId],
  );

  const loadServerConfig = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      await loadMachineFromDisk(server?.id ?? null);
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setIsLoading(false);
    }
  }, [loadMachineFromDisk, server]);

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
    return config.machines.some((entry) => entry.id === nextId && entry.id !== originalId);
  }, [config, form.id, originalId]);

  const updateField = <K extends keyof MachineConfig>(field: K, value: MachineConfig[K]) => {
    setForm((prev) => ({ ...prev, [field]: value }));
    setError(null);
    setSaveMessage(null);
  };

  const updateWorkspaceField = (field: keyof MachineConfig["workspace"], value: string) => {
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

  const updateContainerField = <K extends keyof MachineConfig["containers"][number]>(
    index: number,
    field: K,
    value: MachineConfig["containers"][number][K],
  ) => {
    setForm((prev) => ({
      ...prev,
      containers: prev.containers.map((container, containerIndex) =>
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
      setError("Machine ID is required.");
      return;
    }
    if (hasIdConflict) {
      setError(`Machine "${nextId}" already exists.`);
      return;
    }

    setIsSaving(true);
    setError(null);
    setSaveMessage(null);

    try {
      const nextConfig: TargetsConfigDocument = {
        ...config,
        machines: [...config.machines],
      };
      const hostContainer =
        form.containers.find((container) => container.kind === "host") ?? {
          ...form.containers[0],
          id: "host",
          name: "host",
          kind: "host" as const,
          enabled: true,
          label: "Host",
        };
      const nextServer: MachineConfig = {
        ...form,
        id: nextId,
        containers: [
          {
            ...hostContainer,
            enabled: true,
          },
          ...form.containers.filter(
            (container) => container.kind === "managed" && container.name.trim(),
          ),
        ],
      };

      const existingIndex = nextConfig.machines.findIndex(
        (entry) => entry.id === originalId,
      );

      const isNewServer = existingIndex < 0;

      if (existingIndex >= 0) {
        nextConfig.machines[existingIndex] = nextServer;
      } else {
        nextConfig.machines.push(nextServer);
      }

      await updateTargetsConfig(stringifyTargetsConfig(nextConfig));
      setConfig(nextConfig);
      setOriginalId(nextServer.id);
      setForm(nextServer);

      if (isNewServer && nextServer.discoveryMode === "auto") {
        setIsScanning(true);
        await scanMachineInventory(nextServer.id);
        await loadMachineFromDisk(nextServer.id);
        setSaveMessage("Saved and scanned successfully.");
      } else {
        setSaveMessage("Saved successfully.");
      }
      onSaved();
    } catch (saveError) {
      setError(String(saveError));
    } finally {
      setIsScanning(false);
      setIsSaving(false);
    }
  };

  const handleScan = async () => {
    const serverId = form.id.trim();
    if (!serverId) {
      setError("Save the server first so it has an ID.");
      return;
    }

    setIsScanning(true);
    setError(null);
    setSaveMessage(null);

    try {
      await scanMachineInventory(serverId);
      await loadMachineFromDisk(serverId);
      setSaveMessage("Scanned containers successfully.");
      onSaved();
    } catch (scanError) {
      setError(String(scanError));
    } finally {
      setIsScanning(false);
    }
  };

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div
        aria-label={isCreateMode ? "Create machine" : `${form.label || form.id} machine settings`}
        className="dialog-panel"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
      >
        <header className="dialog-header">
          <div className="dialog-header-copy">
            <span className="dialog-title">{isCreateMode ? "New machine" : form.label || form.id}</span>
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

        <Tabs
          className="dialog-tabs-root"
          onValueChange={(value) => setActiveTab(value as ServerDialogTab)}
          value={activeTab}
        >
          <TabsList className="dialog-tabs" aria-label="Machine settings sections">
            <TabsTrigger className="dialog-tab" value="general">General</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="connection">Connection</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="discovery">Discovery</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="containers">
              Containers
              {form.discoveryMode === "manual"
                ? ` (${form.containers.filter((container) => container.kind === "managed").length})`
                : server
                  ? ` (${server.containers.length})`
                  : ""}
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
                    <SelectField
                      label="Transport"
                      onChange={(value) => updateField("transport", value as MachineConfig["transport"])}
                      options={[
                        { value: "ssh", label: "SSH" },
                        { value: "local", label: "Local" },
                      ]}
                      value={form.transport}
                    />
                    <EditField
                      label="SSH User"
                      mono
                      onChange={(value) => updateField("sshUser", value)}
                      value={form.sshUser}
                    />
                    <EditField
                      label="Container SSH User"
                      mono
                      onChange={(value) => updateField("containerSshUser", value)}
                      value={form.containerSshUser}
                    />
                    <EditField
                      label="Port"
                      mono
                      onChange={(value) => updateField("port", Number(value) || 22)}
                      value={String(form.port)}
                    />
                    <EditField
                      label="Identity Files"
                      mono
                      multiline
                      onChange={(value) =>
                        updateField(
                          "identityFiles",
                          value
                            .split("\n")
                            .map((line) => line.trim())
                            .filter(Boolean),
                        )
                      }
                      value={form.identityFiles.join("\n")}
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
                      label="Discovery mode"
                      onChange={(value) => updateField("discoveryMode", value as ContainerDiscoveryMode)}
                      options={[
                        { value: "host-only", label: "Host only" },
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
                        <Button
                          onClick={() =>
                            updateField("containers", [
                              ...form.containers,
                              createEmptyMachineContainer(),
                            ])
                          }
                          size="sm"
                          variant="outline"
                        >
                          Add container
                        </Button>
                      </div>
                      {form.containers.filter((container) => container.kind === "managed").length === 0 ? (
                        <p className="dialog-empty">No manual containers defined.</p>
                      ) : (
                        <div className="dialog-list">
                          {form.containers
                            .map((container, index) => ({ container, index }))
                            .filter(({ container }) => container.kind === "managed")
                            .map(({ container, index }) => (
                            <div className="dialog-card" key={`${container.name || "container"}-${index}`}>
                              <div className="dialog-card-header">
                                <span className="dialog-card-title">{container.label || container.name || `Container ${index + 1}`}</span>
                                <Button
                                  onClick={() =>
                                    updateField(
                                      "containers",
                                      form.containers.filter((_, itemIndex) => itemIndex !== index),
                                    )
                                  }
                                  size="sm"
                                  variant="ghost"
                                >
                                  Remove
                                </Button>
                              </div>
                              <div className="dialog-field-grid">
                                <EditField label="ID" mono onChange={(value) => updateContainerField(index, "id", value)} value={container.id} />
                                <EditField label="Name" mono onChange={(value) => updateContainerField(index, "name", value)} value={container.name} />
                                <EditField label="Label" onChange={(value) => updateContainerField(index, "label", value)} value={container.label} />
                                <EditField label="IPv4" mono onChange={(value) => updateContainerField(index, "ipv4", value)} value={container.ipv4} />
                                <EditField label="SSH User" mono onChange={(value) => updateContainerField(index, "sshUser", value)} value={container.sshUser} />
                                <EditField label="Status" mono onChange={(value) => updateContainerField(index, "status", value)} value={container.status} />
                                <ToggleField
                                  checked={container.enabled}
                                  label="Enabled"
                                  onChange={(checked) => updateContainerField(index, "enabled", checked)}
                                />
                              </div>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  ) : server ? (
                    <ContainersTable
                      containers={form.containers}
                      isScanning={isScanning}
                      onScan={() => void handleScan()}
                      onSshUserChange={(index, sshUser) => updateContainerField(index, "sshUser", sshUser)}
                      onToggleEnabled={(index, enabled) => updateContainerField(index, "enabled", enabled)}
                    />
                  ) : (
                    <div className="dialog-tab-content">
                      <p className="dialog-empty">Save the server first. Auto-discovery will run once after creation.</p>
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
          <Button disabled={isLoading || isSaving || isScanning} onClick={() => void handleSave()}>
            {isSaving ? "Saving…" : isScanning ? "Scanning…" : "Save"}
          </Button>
          <Button onClick={onClose} variant="outline">
            Close
          </Button>
        </footer>
      </div>
    </div>
  );
}

function ContainersTable({
  containers,
  isScanning,
  onScan,
  onSshUserChange,
  onToggleEnabled,
}: {
  containers: MachineConfig["containers"];
  isScanning: boolean;
  onScan: () => void;
  onSshUserChange: (index: number, sshUser: string) => void;
  onToggleEnabled: (index: number, enabled: boolean) => void;
}) {
  const managedContainers = containers
    .map((container, index) => ({ container, index }))
    .filter(({ container }) => container.kind === "managed");

  if (managedContainers.length === 0) {
    return (
      <div className="dialog-tab-content">
        <div className="dialog-section-actions">
          <Button disabled={isScanning} onClick={onScan} size="sm" variant="outline">
            {isScanning ? "Scanning…" : "Scan containers"}
          </Button>
        </div>
        <p className="dialog-empty">No containers discovered in config for this machine.</p>
      </div>
    );
  }

  return (
    <div className="dialog-tab-content">
      <div className="dialog-section-actions">
        <Button disabled={isScanning} onClick={onScan} size="sm" variant="outline">
          {isScanning ? "Scanning…" : "Scan containers"}
        </Button>
      </div>
      <table className="dialog-table">
        <thead>
          <tr>
            <th>Enabled</th>
            <th>Source</th>
            <th>Name</th>
            <th>Status</th>
            <th>IPv4</th>
            <th>SSH User</th>
          </tr>
        </thead>
        <tbody>
          {managedContainers.map(({ container, index }) => (
            <tr key={container.id || container.name || index}>
              <td>
                <input
                  checked={container.enabled}
                  onChange={(event) => onToggleEnabled(index, event.target.checked)}
                  type="checkbox"
                />
              </td>
              <td>{formatContainerSource(container.source)}</td>
              <td className="dialog-table-mono">{container.name || container.label || "—"}</td>
              <td>
                <Badge variant={container.status === "RUNNING" ? "success" : "warning"}>
                  {container.status}
                </Badge>
              </td>
              <td className="dialog-table-mono">{container.ipv4 || "—"}</td>
              <td>
                <SshUserInput
                  inputId={`container-ssh-user-${container.id || container.name || index}`}
                  onChange={(sshUser) => onSshUserChange(index, sshUser)}
                  options={container.sshUserCandidates}
                  value={container.sshUser}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function SshUserInput({
  inputId,
  onChange,
  options,
  value,
}: {
  inputId: string;
  onChange: (value: string) => void;
  options: string[];
  value: string;
}) {
  const resolvedOptions = Array.from(
    new Set([value, ...options].filter((candidate) => candidate.trim().length > 0)),
  );
  const listId = `${inputId}-options`;

  return (
    <>
      <input
        className="dialog-input dialog-input-mono"
        list={resolvedOptions.length > 0 ? listId : undefined}
        onChange={(event) => onChange(event.target.value)}
        spellCheck={false}
        type="text"
        value={value}
      />
      {resolvedOptions.length > 0 ? (
        <datalist id={listId}>
          {resolvedOptions.map((candidate) => (
            <option key={candidate} value={candidate} />
          ))}
        </datalist>
      ) : null}
    </>
  );
}

function formatContainerSource(source: string): string {
  switch (source) {
    case "orbstack":
      return "OrbStack";
    case "docker":
      return "Docker";
    case "lxd":
      return "LXD";
    case "manual":
      return "Manual";
    case "host":
      return "Host";
    case "custom":
      return "Custom";
    case "unknown":
      return "Unknown";
    default:
      return source || "Unknown";
  }
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

function ToggleField({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="dialog-field">
      <span className="dialog-field-label">{label}</span>
      <input
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        type="checkbox"
      />
    </label>
  );
}
