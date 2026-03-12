import { useCallback, useEffect, useMemo, useState } from "react";
import type { ManagedMachine } from "../types";
import { visibleContainers } from "../lib/container-visibility";
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
type ServerDialogSaveEvent = {
  close?: boolean;
};

interface ServerDialogProps {
  initialTab?: ServerDialogTab;
  onClose: () => void;
  onSaved: (event?: ServerDialogSaveEvent) => void;
  server?: ManagedMachine | null;
  defaultProfileId?: string;
}

export function ServerDialog({
  initialTab = "general",
  onClose,
  onSaved,
  server,
  defaultProfileId = "",
}: ServerDialogProps) {
  const [activeTab, setActiveTab] = useState<ServerDialogTab>(initialTab);
  const [config, setConfig] = useState<TargetsConfigDocument | null>(null);
  const [form, setForm] = useState<MachineConfig>(createEmptyMachine(defaultProfileId));
  const [originalId, setOriginalId] = useState<string | null>(server?.id ?? null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isDeleteConfirming, setIsDeleteConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);
  const [isScanning, setIsScanning] = useState(false);

  const isCreateMode = !server;
  const canDeleteMachine = !isCreateMode && originalId !== "local";

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

  useEffect(() => {
    setActiveTab(initialTab);
  }, [initialTab, server?.id]);

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

  const persistCurrentMachineConfig = async (): Promise<{
    nextConfig: TargetsConfigDocument;
    nextServer: MachineConfig;
    isNewServer: boolean;
  }> => {
    if (!config) {
      throw new Error("Provider config is not loaded.");
    }

    const nextId = form.id.trim();
    if (!nextId) {
      throw new Error("Provider ID is required.");
    }
    if (hasIdConflict) {
      throw new Error(`Provider "${nextId}" already exists.`);
    }

    const nextConfig: TargetsConfigDocument = {
      ...config,
      machines: [...config.machines],
    };
    const nextServer: MachineConfig = {
      ...form,
      id: nextId,
      containers: form.containers.filter(
        (container) => container.kind === "managed" && container.name.trim(),
      ),
    };

    const existingIndex = nextConfig.machines.findIndex((entry) => entry.id === originalId);
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

    return { nextConfig, nextServer, isNewServer };
  };

  const updateField = <K extends keyof MachineConfig>(field: K, value: MachineConfig[K]) => {
    setForm((prev) => ({ ...prev, [field]: value }));
    setIsDeleteConfirming(false);
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
    setIsDeleteConfirming(false);
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
    setIsDeleteConfirming(false);
    setError(null);
    setSaveMessage(null);
  };

  const updateContainerWorkspaceField = (
    index: number,
    field: keyof MachineConfig["containers"][number]["workspace"],
    value: string,
  ) => {
    setForm((prev) => ({
      ...prev,
      containers: prev.containers.map((container, containerIndex) =>
        containerIndex === index
          ? {
              ...container,
              workspace: {
                ...container.workspace,
                [field]: value,
              },
            }
          : container,
      ),
    }));
    setIsDeleteConfirming(false);
    setError(null);
    setSaveMessage(null);
  };

  const clearContainerProfileOverrides = () => {
    setForm((prev) => ({
      ...prev,
      containers: prev.containers.map((container) =>
        container.kind === "managed"
          ? {
              ...container,
              workspace: {
                ...container.workspace,
                profile: "",
              },
            }
          : container,
      ),
    }));
    setIsDeleteConfirming(false);
    setError(null);
    setSaveMessage(null);
  };

  const handleSave = async () => {
    if (!config) return;

    setIsSaving(true);
    setError(null);
    setSaveMessage(null);

    try {
      const { nextServer, isNewServer } = await persistCurrentMachineConfig();

      if (isNewServer && nextServer.discoveryMode === "auto") {
        setIsScanning(true);
        await scanMachineInventory(nextServer.id);
        await loadMachineFromDisk(nextServer.id);
        setSaveMessage("Saved and scanned successfully.");
      } else {
        setSaveMessage("Saved successfully.");
      }
      onSaved({ close: true });
    } catch (saveError) {
      setError(String(saveError));
    } finally {
      setIsScanning(false);
      setIsSaving(false);
    }
  };

  const handleScan = async () => {
    setIsScanning(true);
    setError(null);
    setSaveMessage(null);

    try {
      const { nextServer } = await persistCurrentMachineConfig();
      await scanMachineInventory(nextServer.id);
      await loadMachineFromDisk(nextServer.id);
      setSaveMessage("Scanned containers successfully.");
      onSaved({ close: false });
    } catch (scanError) {
      setError(scanError instanceof Error ? scanError.message : String(scanError));
    } finally {
      setIsScanning(false);
    }
  };

  const handleDelete = async () => {
    if (!config || !originalId || !canDeleteMachine) {
      return;
    }

    if (!isDeleteConfirming) {
      setIsDeleteConfirming(true);
      return;
    }

    setIsDeleting(true);
    setIsDeleteConfirming(false);
    setError(null);
    setSaveMessage(null);

    try {
      const nextConfig: TargetsConfigDocument = {
        ...config,
        machines: config.machines.filter((entry) => entry.id !== originalId),
      };

      await updateTargetsConfig(stringifyTargetsConfig(nextConfig));
      onSaved({ close: true });
      onClose();
    } catch (deleteError) {
      setError(String(deleteError));
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div
        aria-label={isCreateMode ? "Create provider" : `${form.label || form.id} provider settings`}
        className="dialog-panel dialog-panel-wide"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
      >
        <header className="dialog-header">
          <div className="dialog-header-copy">
            <span className="dialog-title">{isCreateMode ? "New provider" : form.label || form.id}</span>
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
          <TabsList className="dialog-tabs" aria-label="Provider settings sections">
            <TabsTrigger className="dialog-tab" value="general">General</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="connection">Connection</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="discovery">Discovery</TabsTrigger>
                <TabsTrigger className="dialog-tab" value="containers">
              Containers
              {form.discoveryMode === "manual"
                ? ` (${form.containers.filter((container) => container.kind === "managed").length})`
                : server
                  ? ` (${visibleContainers(server.containers).length})`
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
                      hint="Used as the default for this provider and for containers that do not override it."
                      label="Provider Profile"
                      onChange={(value) => updateWorkspaceField("profile", value)}
                      options={profileOptions.map((profile) => ({ value: profile, label: profile }))}
                      emptyLabel="No profile"
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
                      label="Host Terminal Startup Command"
                      multiline
                      mono
                      onChange={(value) => updateField("hostTerminalStartupCommand", value)}
                      value={form.hostTerminalStartupCommand}
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
                                <SelectField
                                  label="Profile Override"
                                  onChange={(value) => updateContainerWorkspaceField(index, "profile", value)}
                                  options={profileOptions.map((profile) => ({ value: profile, label: profile }))}
                                  emptyLabel={
                                    form.workspace.profile
                                      ? `Inherit provider profile (${form.workspace.profile})`
                                      : "Inherit provider profile"
                                  }
                                  value={container.workspace.profile}
                                />
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
                      machineProfileId={form.workspace.profile}
                      onClearProfileOverrides={clearContainerProfileOverrides}
                      profileOptions={profileOptions}
                      onProfileChange={(index, profile) => updateContainerWorkspaceField(index, "profile", profile)}
                      onScan={() => void handleScan()}
                      onSshUserChange={(index, sshUser) => updateContainerField(index, "sshUser", sshUser)}
                      onToggleEnabled={(index, enabled) => updateContainerField(index, "enabled", enabled)}
                    />
                  ) : (
                    <div className="dialog-tab-content">
                      <p className="dialog-empty">Save the provider first. Auto-discovery will run once after creation.</p>
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
          {canDeleteMachine ? (
            <div className="dialog-footer-danger-group">
              {isDeleteConfirming ? (
                <span className="dialog-footer-danger-copy">
                  Delete this provider from config?
                </span>
              ) : null}
              <Button
                className="dialog-footer-danger"
                disabled={isLoading || isSaving || isScanning || isDeleting}
                onClick={() => void handleDelete()}
                variant="ghost"
              >
                {isDeleting
                  ? "Deleting…"
                  : isDeleteConfirming
                    ? "Confirm delete"
                    : "Delete provider"}
              </Button>
              {isDeleteConfirming ? (
                <Button
                  disabled={isDeleting}
                  onClick={() => setIsDeleteConfirming(false)}
                  variant="outline"
                >
                  Cancel
                </Button>
              ) : null}
            </div>
          ) : null}
          <Button disabled={isLoading || isSaving || isScanning || isDeleting} onClick={() => void handleSave()}>
            {isSaving ? "Saving…" : isScanning ? "Scanning…" : isDeleting ? "Deleting…" : "Save"}
          </Button>
          <Button disabled={isDeleting} onClick={onClose} variant="outline">
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
  machineProfileId,
  onClearProfileOverrides,
  profileOptions,
  onProfileChange,
  onScan,
  onSshUserChange,
  onToggleEnabled,
}: {
  containers: MachineConfig["containers"];
  isScanning: boolean;
  machineProfileId: string;
  onClearProfileOverrides: () => void;
  profileOptions: string[];
  onProfileChange: (index: number, profile: string) => void;
  onScan: () => void;
  onSshUserChange: (index: number, sshUser: string) => void;
  onToggleEnabled: (index: number, enabled: boolean) => void;
}) {
  const managedContainers = containers
    .map((container, index) => ({ container, index }))
    .filter(({ container }) => container.kind === "managed");
  const hasProfileOverrides = managedContainers.some(
    ({ container }) => container.workspace.profile.trim().length > 0,
  );

  if (managedContainers.length === 0) {
    return (
      <div className="dialog-tab-content">
        <div className="dialog-section-actions">
          <Button disabled={isScanning} onClick={onScan} size="sm" variant="outline">
            {isScanning ? "Scanning…" : "Scan containers"}
          </Button>
        </div>
        <p className="dialog-empty">No containers discovered in config for this provider.</p>
      </div>
    );
  }

  return (
    <div className="dialog-tab-content">
      <div className="dialog-section-actions">
        <Button disabled={isScanning} onClick={onScan} size="sm" variant="outline">
          {isScanning ? "Scanning…" : "Scan containers"}
        </Button>
        <Button
          disabled={!hasProfileOverrides}
          onClick={onClearProfileOverrides}
          size="sm"
          variant="ghost"
        >
          Use provider profile
        </Button>
      </div>
      <p className="dialog-section-copy">
        Containers inherit the provider profile by default. Set a container profile only when it needs to override
        {machineProfileId ? ` ${machineProfileId}` : " the provider profile"}.
      </p>
      <div className="dialog-table-scroll">
        <table className="dialog-table">
          <thead>
            <tr>
              <th>Enabled</th>
              <th>Source</th>
              <th>Name</th>
              <th>Status</th>
              <th>IPv4</th>
              <th>Profile Override</th>
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
                  <ProfileSelectInput
                    machineProfileId={machineProfileId}
                    onChange={(profile) => onProfileChange(index, profile)}
                    options={profileOptions}
                    value={container.workspace.profile}
                  />
                </td>
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
    </div>
  );
}

function ProfileSelectInput({
  machineProfileId,
  onChange,
  options,
  value,
}: {
  machineProfileId: string;
  onChange: (value: string) => void;
  options: string[];
  value: string;
}) {
  return (
    <select className="dialog-input" onChange={(event) => onChange(event.target.value)} value={value}>
      <option value="">
        {machineProfileId
          ? `Inherit provider profile (${machineProfileId})`
          : "Inherit provider profile"}
      </option>
      {options.map((option) => (
        <option key={option} value={option}>
          {option}
        </option>
      ))}
    </select>
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
  emptyLabel,
  hint,
  label,
  onChange,
  options,
  value,
}: {
  emptyLabel?: string;
  hint?: string;
  label: string;
  onChange: (value: string) => void;
  options: Array<{ value: string; label: string }>;
  value: string;
}) {
  return (
    <div className="dialog-field-group">
      <div className="dialog-field">
      <label className="dialog-field-label">{label}</label>
      <select className="dialog-input" onChange={(event) => onChange(event.target.value)} value={value}>
        <option value="">{emptyLabel ?? "Select…"}</option>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      </div>
      {hint ? <p className="dialog-field-hint">{hint}</p> : null}
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
