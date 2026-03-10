import { useEffect, useMemo, useState } from "react";
import { loadTargetsConfig, updateTargetsConfig } from "../lib/tauri";
import {
  createEmptyProfile,
  parseTargetsConfig,
  stringifyTargetsConfig,
  type ContainerProfileConfig,
  type TargetsConfigDocument,
} from "../lib/targets-config";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";

type ProfileDialogTab = "workspace" | "services";

interface ProfileDialogProps {
  onClose: () => void;
  onSaved: () => void;
  profileId?: string | null;
}

function serializeServices(profile: ContainerProfileConfig): string {
  return JSON.stringify(profile.services ?? [], null, 2);
}

export function ProfileDialog({ onClose, onSaved, profileId }: ProfileDialogProps) {
  const [activeTab, setActiveTab] = useState<ProfileDialogTab>("workspace");
  const [config, setConfig] = useState<TargetsConfigDocument | null>(null);
  const [currentProfileId, setCurrentProfileId] = useState(profileId ?? "");
  const [form, setForm] = useState<ContainerProfileConfig>(createEmptyProfile(profileId ?? ""));
  const [servicesJson, setServicesJson] = useState<string>(serializeServices(createEmptyProfile(profileId ?? "")));
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);

  const isCreateMode = !profileId;

  useEffect(() => {
    void (async () => {
      try {
        const raw = await loadTargetsConfig();
        const parsed = parseTargetsConfig(raw);
        setConfig(parsed);

        if (profileId && parsed.profiles[profileId]) {
          const next = parsed.profiles[profileId];
          setCurrentProfileId(profileId);
          setForm(next);
          setServicesJson(serializeServices(next));
        } else {
          const next = createEmptyProfile(profileId ?? "");
          setForm(next);
          setServicesJson(serializeServices(next));
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

  const updateWorkspaceField = (field: keyof ContainerProfileConfig["workspace"], value: string) => {
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

    let parsedServices;
    try {
      parsedServices = JSON.parse(servicesJson);
      if (!Array.isArray(parsedServices)) {
        throw new Error("Services must be a JSON array.");
      }
    } catch (parseError) {
      setError(`Invalid services JSON: ${String(parseError)}`);
      return;
    }

    setIsSaving(true);
    setError(null);
    setSaveMessage(null);

    try {
      const nextConfig = { ...config, profiles: { ...config.profiles } };
      if (profileId && profileId !== nextProfileId) {
        delete nextConfig.profiles[profileId];
      }

      nextConfig.profiles[nextProfileId] = {
        ...form,
        workspace: {
          ...form.workspace,
          profile: nextProfileId,
        },
        services: parsedServices,
      };

      await updateTargetsConfig(stringifyTargetsConfig(nextConfig));
      setConfig(nextConfig);
      setCurrentProfileId(nextProfileId);
      setForm(nextConfig.profiles[nextProfileId]);
      setServicesJson(JSON.stringify(parsedServices, null, 2));
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
        aria-label={isCreateMode ? "Create profile" : `${currentProfileId} profile settings`}
        className="dialog-panel"
        onClick={(event) => event.stopPropagation()}
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

        <Tabs onValueChange={(value) => setActiveTab(value as ProfileDialogTab)} value={activeTab}>
          <TabsList className="dialog-tabs" aria-label="Profile settings sections">
            <TabsTrigger className="dialog-tab" value="workspace">Workspace</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="services">Services</TabsTrigger>
          </TabsList>

          <div className="dialog-body">
            {isLoading ? (
              <p className="dialog-empty">Loading configuration…</p>
            ) : (
              <>
                <TabsContent value="workspace">
                  <div className="dialog-tab-content">
                    <EditField label="Profile ID" mono onChange={setCurrentProfileId} value={currentProfileId} />
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
                <TabsContent value="services">
                  <div className="dialog-tab-content">
                    <EditField
                      label="Services JSON"
                      mono
                      multiline
                      onChange={setServicesJson}
                      value={servicesJson}
                    />
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
          rows={14}
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
