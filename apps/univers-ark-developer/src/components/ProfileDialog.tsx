import { useEffect, useMemo, useRef, useState } from "react";
import { loadMachineConfigState, upsertProfileConfig } from "../lib/tauri";
import {
  createDefaultCommandService,
  createDefaultEndpointService,
  createDefaultWebService,
  createEmptyProfile,
  type ContainerProfileConfig,
  type EditableDeveloperService,
  type TargetsConfigDocument,
} from "../lib/targets-config";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";

type ProfileDialogTab = "workspace" | "services";
type ServiceKind = EditableDeveloperService["kind"];

interface ProfileDialogProps {
  onClose: () => void;
  onSaved: () => void;
  profileId?: string | null;
}

function createServiceForKind(kind: ServiceKind): EditableDeveloperService {
  switch (kind) {
    case "endpoint":
      return createDefaultEndpointService("endpoint", "Endpoint");
    case "command":
      return createDefaultCommandService("command", "Command");
    case "web":
    default:
      return createDefaultWebService("web", "Web", "http");
  }
}

function switchServiceKind(
  service: EditableDeveloperService,
  kind: ServiceKind,
): EditableDeveloperService {
  const next = createServiceForKind(kind);
  return {
    ...next,
    id: service.id,
    label: service.label,
    description: service.description,
  };
}

function parseWebUrl(url: string): { host: string; path: string; port: string } {
  try {
    const parsed = new URL(url);
    return {
      host: parsed.hostname,
      path: `${parsed.pathname}${parsed.search}` || "/",
      port:
        parsed.port ||
        (parsed.protocol === "https:" ? "443" : parsed.protocol === "http:" ? "80" : ""),
    };
  } catch {
    return {
      host: "127.0.0.1",
      path: "/",
      port: "",
    };
  }
}

function buildRemoteUrl(
  host: string,
  port: string,
  path: string,
): string {
  const normalizedHost = host.trim() || "127.0.0.1";
  const normalizedPort = port.trim();
  const normalizedPath = path.trim()
    ? path.startsWith("/") ? path : `/${path}`
    : "/";

  return `http://${normalizedHost}${normalizedPort ? `:${normalizedPort}` : ""}${normalizedPath}`;
}

export function ProfileDialog({ onClose, onSaved, profileId }: ProfileDialogProps) {
  const [activeTab, setActiveTab] = useState<ProfileDialogTab>("workspace");
  const [config, setConfig] = useState<TargetsConfigDocument | null>(null);
  const [currentProfileId, setCurrentProfileId] = useState(profileId ?? "");
  const [form, setForm] = useState<ContainerProfileConfig>(createEmptyProfile(profileId ?? ""));
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);
  const shouldCloseFromBackdropRef = useRef(false);

  const isCreateMode = !profileId;

  useEffect(() => {
    void (async () => {
      try {
        const parsed = await loadMachineConfigState();
        setConfig(parsed);

        if (profileId && parsed.profiles[profileId]) {
          const next = parsed.profiles[profileId];
          setCurrentProfileId(profileId);
          setForm(next);
        } else {
          setForm(createEmptyProfile(profileId ?? ""));
        }
      } catch (loadError) {
        setError(String(loadError));
      } finally {
        setIsLoading(false);
      }
    })();
  }, [profileId]);

  const hasProfileConflict = useMemo(() => {
    if (!config) return false;
    const trimmed = currentProfileId.trim();
    if (!trimmed) return false;
    return trimmed !== profileId && Boolean(config.profiles[trimmed]);
  }, [config, currentProfileId, profileId]);

  const updateWorkspaceField = (
    field: keyof ContainerProfileConfig["workspace"],
    value: string,
  ) => {
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

  const updateService = (
    index: number,
    updater: (service: EditableDeveloperService) => EditableDeveloperService,
  ) => {
    setForm((prev) => ({
      ...prev,
      services: prev.services.map((service, serviceIndex) =>
        serviceIndex === index ? updater(service) : service,
      ),
    }));
    setError(null);
    setSaveMessage(null);
  };

  const addService = (kind: ServiceKind) => {
    setForm((prev) => ({
      ...prev,
      services: [
        ...prev.services,
        createServiceForKind(kind),
      ],
    }));
    setError(null);
    setSaveMessage(null);
  };

  const updateWebServiceUrlPart = (
    index: number,
    field: "host" | "port" | "path",
    value: string,
  ) => {
    updateService(index, (current) => {
      if (!current.web) {
        return current;
      }

      const parts = parseWebUrl(current.web.remoteUrl);
      const nextParts = { ...parts, [field]: value };

      return {
        ...current,
        web: {
          ...current.web,
          remoteUrl: buildRemoteUrl(nextParts.host, nextParts.port, nextParts.path),
        },
      };
    });
  };

  const removeService = (index: number) => {
    setForm((prev) => ({
      ...prev,
      services: prev.services.filter((_, serviceIndex) => serviceIndex !== index),
    }));
    setError(null);
    setSaveMessage(null);
  };

  const handleSave = async () => {
    if (!config) return;

    const nextProfileId = currentProfileId.trim();
    if (!nextProfileId) {
      setError("Profile ID is required.");
      return;
    }
    if (hasProfileConflict) {
      setError(`Profile "${nextProfileId}" already exists.`);
      return;
    }

    setIsSaving(true);
    setError(null);
    setSaveMessage(null);

    try {
      const persistedConfig = await upsertProfileConfig(nextProfileId, {
        ...form,
        extends: form.extends?.trim() ?? "",
        workspace: {
          ...form.workspace,
          profile: nextProfileId,
        },
      }, profileId);
      setConfig(persistedConfig);
      setCurrentProfileId(nextProfileId);
      setForm(persistedConfig.profiles[nextProfileId]);
      setSaveMessage("Saved successfully.");
      onSaved();
    } catch (saveError) {
      setError(String(saveError));
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div
      className="dialog-backdrop"
      onClick={(event) => {
        if (
          shouldCloseFromBackdropRef.current &&
          event.target === event.currentTarget
        ) {
          onClose();
        }
        shouldCloseFromBackdropRef.current = false;
      }}
      onPointerDown={(event) => {
        shouldCloseFromBackdropRef.current =
          event.target === event.currentTarget;
      }}
    >
      <div
        aria-label={isCreateMode ? "Create profile" : `${currentProfileId} profile settings`}
        className="dialog-panel"
        onClick={(event) => event.stopPropagation()}
        onPointerDown={() => {
          shouldCloseFromBackdropRef.current = false;
        }}
        role="dialog"
      >
        <header className="dialog-header">
          <div className="dialog-header-copy">
            <span className="dialog-title">{isCreateMode ? "New profile" : currentProfileId}</span>
            <span className="dialog-subtitle">Workspace defaults and declared services</span>
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
          onValueChange={(value) => setActiveTab(value as ProfileDialogTab)}
          value={activeTab}
        >
          <TabsList className="dialog-tabs" aria-label="Profile settings sections">
            <TabsTrigger className="dialog-tab" value="workspace">Workspace</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="services">Services</TabsTrigger>
          </TabsList>

          <div className="dialog-body">
            {isLoading ? (
              <p className="dialog-empty">Loading configuration…</p>
            ) : (
              <>
                <TabsContent className="dialog-tab-panel" value="workspace">
                  <div className="dialog-tab-content">
                    <EditField label="Profile ID" mono onChange={setCurrentProfileId} value={currentProfileId} />
                    <EditField
                      label="Extends"
                      mono
                      onChange={(value) =>
                        setForm((prev) => ({
                          ...prev,
                          extends: value,
                        }))
                      }
                      value={form.extends ?? ""}
                    />
                    <EditField
                      label="Default Tool"
                      onChange={(value) => updateWorkspaceField("defaultTool", value)}
                      value={form.workspace.defaultTool}
                    />
                    <EditField
                      label="Project Path"
                      mono
                      onChange={(value) => updateWorkspaceField("projectPath", value)}
                      value={form.workspace.projectPath}
                    />
                    <EditField
                      label="Files Root"
                      mono
                      onChange={(value) => updateWorkspaceField("filesRoot", value)}
                      value={form.workspace.filesRoot}
                    />
                    <EditField
                      label="Primary Web Service"
                      mono
                      onChange={(value) => updateWorkspaceField("primaryWebServiceId", value)}
                      value={form.workspace.primaryWebServiceId ?? ""}
                    />
                    <EditField
                      label="Tmux Command Service"
                      mono
                      onChange={(value) => updateWorkspaceField("tmuxCommandServiceId", value)}
                      value={form.workspace.tmuxCommandServiceId}
                    />
                  </div>
                </TabsContent>
                <TabsContent className="dialog-tab-panel" value="services">
                  <div className="dialog-tab-content">
                    <div className="dialog-section-actions">
                      <span className="dialog-field-label">Declared services</span>
                      <div className="settings-option-group">
                        <Button onClick={() => addService("web")} size="sm" variant="outline">
                          Add web
                        </Button>
                        <Button onClick={() => addService("endpoint")} size="sm" variant="outline">
                          Add endpoint
                        </Button>
                        <Button onClick={() => addService("command")} size="sm" variant="outline">
                          Add command
                        </Button>
                      </div>
                    </div>
                    {form.services.length === 0 ? (
                      <p className="dialog-empty">No services declared.</p>
                    ) : (
                      <div className="dialog-list">
                        {form.services.map((service, index) => (
                          <div className="dialog-card" key={`${service.id || "service"}-${index}`}>
                            <div className="dialog-card-header">
                              <span className="dialog-card-title">
                                {service.label || service.id || `Service ${index + 1}`}
                              </span>
                              <Button
                                onClick={() => removeService(index)}
                                size="sm"
                                variant="outline"
                              >
                                Remove
                              </Button>
                            </div>
                            <div className="dialog-tab-content">
                              <EditField
                                label="ID"
                                mono
                                onChange={(value) =>
                                  updateService(index, (current) => ({ ...current, id: value }))
                                }
                                value={service.id}
                              />
                              <EditField
                                label="Label"
                                onChange={(value) =>
                                  updateService(index, (current) => ({ ...current, label: value }))
                                }
                                value={service.label}
                              />
                              <SelectField
                                label="Kind"
                                onChange={(value) =>
                                  updateService(index, (current) =>
                                    switchServiceKind(current, value as ServiceKind),
                                  )
                                }
                                options={[
                                  ["web", "Web"],
                                  ["endpoint", "Endpoint"],
                                  ["command", "Command"],
                                ]}
                                value={service.kind}
                              />
                              <EditField
                                label="Description"
                                onChange={(value) =>
                                  updateService(index, (current) => ({
                                    ...current,
                                    description: value,
                                  }))
                                }
                                value={service.description ?? ""}
                              />

                              {service.kind === "web" && service.web ? (
                                <>
                                  {(() => {
                                    const remoteParts = parseWebUrl(service.web.remoteUrl);
                                    return (
                                      <>
                                        <EditField
                                          label="Host"
                                          mono
                                          onChange={(value) =>
                                            updateWebServiceUrlPart(index, "host", value)
                                          }
                                          value={remoteParts.host}
                                        />
                                        <EditField
                                          label="Port"
                                          mono
                                          onChange={(value) =>
                                            updateWebServiceUrlPart(index, "port", value)
                                          }
                                          value={remoteParts.port}
                                        />
                                        <EditField
                                          label="Path"
                                          mono
                                          onChange={(value) =>
                                            updateWebServiceUrlPart(index, "path", value)
                                          }
                                          value={remoteParts.path}
                                        />
                                      </>
                                    );
                                  })()}
                                  <SelectField
                                    label="Web Type"
                                    onChange={(value) =>
                                      updateService(index, (current) => ({
                                        ...current,
                                        web: current.web
                                          ? { ...current.web, serviceType: value as "http" | "vite" }
                                          : current.web,
                                      }))
                                    }
                                    options={[
                                      ["http", "HTTP"],
                                      ["vite", "Vite"],
                                    ]}
                                    value={service.web.serviceType}
                                  />
                                  <ToggleField
                                    checked={Boolean(service.web.backgroundPrerender)}
                                    hint="Warm the tunnel and preload this service in the background after the app starts."
                                    label="Background prerender on app startup"
                                    onChange={(checked) =>
                                      updateService(index, (current) => ({
                                        ...current,
                                        web: current.web
                                          ? { ...current.web, backgroundPrerender: checked }
                                          : current.web,
                                      }))
                                    }
                                  />
                                  <ReadOnlyField
                                    label="Resolved Remote URL"
                                    mono
                                    value={service.web.remoteUrl}
                                  />
                                  <details className="dialog-card-advanced">
                                    <summary className="dialog-card-advanced-summary">
                                      Advanced
                                    </summary>
                                    <div className="dialog-card-advanced-body">
                                      <EditField
                                        label="Local URL Template"
                                        mono
                                        onChange={(value) =>
                                          updateService(index, (current) => ({
                                            ...current,
                                            web: current.web ? { ...current.web, localUrl: value } : current.web,
                                          }))
                                        }
                                        value={service.web.localUrl}
                                      />
                                      <EditField
                                        label="Tunnel Command"
                                        mono
                                        onChange={(value) =>
                                          updateService(index, (current) => ({
                                            ...current,
                                            web: current.web
                                              ? { ...current.web, tunnelCommand: value }
                                              : current.web,
                                          }))
                                        }
                                        value={service.web.tunnelCommand}
                                      />
                                      {service.web.serviceType === "vite" ? (
                                        <EditField
                                          label="Vite HMR Tunnel Command"
                                          mono
                                          onChange={(value) =>
                                            updateService(index, (current) => ({
                                              ...current,
                                              web: current.web
                                                ? { ...current.web, viteHmrTunnelCommand: value }
                                                : current.web,
                                            }))
                                          }
                                          value={service.web.viteHmrTunnelCommand ?? ""}
                                        />
                                      ) : null}
                                    </div>
                                  </details>
                                </>
                              ) : null}

                              {service.kind === "endpoint" && service.endpoint ? (
                                <>
                                  <SelectField
                                    label="Probe Type"
                                    onChange={(value) =>
                                      updateService(index, (current) => ({
                                        ...current,
                                        endpoint: current.endpoint
                                          ? {
                                              ...current.endpoint,
                                              probeType: value as "http" | "tcp",
                                            }
                                          : current.endpoint,
                                      }))
                                    }
                                    options={[
                                      ["http", "HTTP"],
                                      ["tcp", "TCP"],
                                    ]}
                                    value={service.endpoint.probeType}
                                  />
                                  <EditField
                                    label="Host"
                                    mono
                                    onChange={(value) =>
                                      updateService(index, (current) => ({
                                        ...current,
                                        endpoint: current.endpoint
                                          ? { ...current.endpoint, host: value }
                                          : current.endpoint,
                                      }))
                                    }
                                    value={service.endpoint.host}
                                  />
                                  <EditField
                                    label="Port"
                                    mono
                                    onChange={(value) =>
                                      updateService(index, (current) => ({
                                        ...current,
                                        endpoint: current.endpoint
                                          ? {
                                              ...current.endpoint,
                                              port: Number.parseInt(value, 10) || 0,
                                            }
                                          : current.endpoint,
                                      }))
                                    }
                                    value={String(service.endpoint.port ?? 0)}
                                  />
                                  <EditField
                                    label="Path"
                                    mono
                                    onChange={(value) =>
                                      updateService(index, (current) => ({
                                        ...current,
                                        endpoint: current.endpoint
                                          ? { ...current.endpoint, path: value }
                                          : current.endpoint,
                                      }))
                                    }
                                    value={service.endpoint.path}
                                  />
                                  <EditField
                                    label="URL"
                                    mono
                                    onChange={(value) =>
                                      updateService(index, (current) => ({
                                        ...current,
                                        endpoint: current.endpoint
                                          ? { ...current.endpoint, url: value }
                                          : current.endpoint,
                                      }))
                                    }
                                    value={service.endpoint.url}
                                  />
                                </>
                              ) : null}

                              {service.kind === "command" && service.command ? (
                                <EditField
                                  label="Restart Command"
                                  mono
                                  multiline
                                  onChange={(value) =>
                                    updateService(index, (current) => ({
                                      ...current,
                                      command: current.command
                                        ? { ...current.command, restart: value }
                                        : current.command,
                                    }))
                                  }
                                  value={service.command.restart}
                                />
                              ) : null}
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
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
          rows={6}
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

function ReadOnlyField({
  label,
  mono,
  value,
}: {
  label: string;
  mono?: boolean;
  value: string;
}) {
  const className = `dialog-input ${mono ? "dialog-input-mono" : ""}`;

  return (
    <div className="dialog-field">
      <label className="dialog-field-label">{label}</label>
      <input
        className={`${className} dialog-input-readonly`}
        readOnly
        spellCheck={false}
        type="text"
        value={value}
      />
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
  options: Array<[string, string]>;
  value: string;
}) {
  return (
    <div className="dialog-field">
      <label className="dialog-field-label">{label}</label>
      <select
        className="dialog-input"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {options.map(([optionValue, optionLabel]) => (
          <option key={optionValue} value={optionValue}>
            {optionLabel}
          </option>
        ))}
      </select>
    </div>
  );
}

function ToggleField({
  checked,
  hint,
  label,
  onChange,
}: {
  checked: boolean;
  hint?: string;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="dialog-field-group">
      <label className="dialog-field">
        <span className="dialog-field-label">{label}</span>
        <input
          checked={checked}
          className="dialog-choice-checkbox"
          onChange={(event) => onChange(event.target.checked)}
          type="checkbox"
        />
      </label>
      {hint ? <p className="dialog-field-hint">{hint}</p> : null}
    </div>
  );
}
