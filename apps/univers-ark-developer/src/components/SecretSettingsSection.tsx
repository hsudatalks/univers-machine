import { useEffect, useMemo, useState } from "react";
import { visibleContainers } from "../lib/container-visibility";
import {
  deleteSecretAssignment,
  deleteSecretCredential,
  deleteSecretProvider,
  loadSecretInventory,
  upsertSecretAssignment,
  upsertSecretCredential,
  upsertSecretProvider,
} from "../lib/tauri";
import type {
  ManagedMachine,
  SecretAssignmentInput,
  SecretAssignmentRecord,
  SecretAssignmentTargetKind,
  SecretCredentialInput,
  SecretCredentialRecord,
  SecretInventory,
  SecretProviderInput,
  SecretProviderRecord,
} from "../types";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";

const PROVIDER_KIND_OPTIONS = [
  { label: "OpenAI", value: "openai" },
  { label: "Anthropic", value: "anthropic" },
  { label: "Google", value: "google" },
  { label: "Custom", value: "custom" },
] as const;

function emptyProviderForm(): SecretProviderInput {
  return {
    label: "",
    providerKind: "openai",
    baseUrl: "",
    description: "",
  };
}

function emptyCredentialForm(): SecretCredentialInput {
  return {
    providerId: "",
    label: "",
    description: "",
    secretValue: "",
    clearSecret: false,
  };
}

function emptyAssignmentForm(): SecretAssignmentInput {
  return {
    credentialId: "",
    targetKind: "machine",
    targetId: "",
    envVar: "",
    filePath: "",
    enabled: true,
  };
}

function formatTimestamp(timestampMs: number | null | undefined): string {
  if (!timestampMs) {
    return "Never";
  }

  return new Date(timestampMs).toLocaleString();
}

interface SecretSettingsSectionProps {
  active: boolean;
  machines: ManagedMachine[];
}

type SecretSettingsTab = "providers" | "credentials" | "assignments" | "audit";

export function SecretSettingsSection({
  active,
  machines,
}: SecretSettingsSectionProps) {
  const [activeTab, setActiveTab] = useState<SecretSettingsTab>("providers");
  const [inventory, setInventory] = useState<SecretInventory | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [providerForm, setProviderForm] = useState<SecretProviderInput>(
    emptyProviderForm(),
  );
  const [credentialForm, setCredentialForm] = useState<SecretCredentialInput>(
    emptyCredentialForm(),
  );
  const [assignmentForm, setAssignmentForm] = useState<SecretAssignmentInput>(
    emptyAssignmentForm(),
  );

  const refreshInventory = async (showBusy = true) => {
    if (showBusy) {
      setIsLoading(true);
    }

    try {
      const nextInventory = await loadSecretInventory();
      setInventory(nextInventory);
      setError(null);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      if (showBusy) {
        setIsLoading(false);
      }
    }
  };

  useEffect(() => {
    if (!active) {
      return;
    }

    void refreshInventory(true);
  }, [active]);

  const providerMap = useMemo(
    () =>
      new Map(
        (inventory?.providers ?? []).map((provider) => [provider.id, provider]),
      ),
    [inventory?.providers],
  );
  const credentialMap = useMemo(
    () =>
      new Map(
        (inventory?.credentials ?? []).map((credential) => [credential.id, credential]),
      ),
    [inventory?.credentials],
  );
  const assignmentTargetOptions = useMemo(() => {
    const machineOptions = machines.map((machine) => ({
      id: machine.id,
      kind: "machine" as const,
      label: machine.label,
      meta: machine.host,
    }));
    const containerOptions = machines.flatMap((machine) =>
      visibleContainers(machine.containers).map((container) => ({
        id: container.targetId,
        kind: "container" as const,
        label: `${machine.label} / ${container.label}`,
        meta: container.targetId,
      })),
    );

    return {
      machine: machineOptions,
      container: containerOptions,
    };
  }, [machines]);
  const targetLabelMap = useMemo(() => {
    const entries = [
      ...assignmentTargetOptions.machine.map((entry) => [entry.id, entry.label] as const),
      ...assignmentTargetOptions.container.map((entry) => [entry.id, entry.label] as const),
    ];

    return new Map(entries);
  }, [assignmentTargetOptions]);

  useEffect(() => {
    if (!inventory) {
      return;
    }

    const firstProviderId = inventory.providers[0]?.id ?? "";
    const nextProviderId = providerMap.has(credentialForm.providerId)
      ? credentialForm.providerId
      : firstProviderId;

    if (credentialForm.providerId !== nextProviderId) {
      setCredentialForm((current) => ({
        ...current,
        providerId: nextProviderId,
      }));
    }

    const nextCredentialId = credentialMap.has(assignmentForm.credentialId)
      ? assignmentForm.credentialId
      : inventory.credentials[0]?.id ?? "";

    if (assignmentForm.credentialId !== nextCredentialId) {
      setAssignmentForm((current) => ({
        ...current,
        credentialId: nextCredentialId,
      }));
    }
  }, [
    assignmentForm.credentialId,
    credentialForm.providerId,
    credentialMap,
    inventory,
    providerMap,
  ]);

  useEffect(() => {
    const targetOptions = assignmentTargetOptions[assignmentForm.targetKind];
    const nextTargetId = targetOptions.some(
      (option) => option.id === assignmentForm.targetId,
    )
      ? assignmentForm.targetId
      : targetOptions[0]?.id ?? "";

    if (assignmentForm.targetId !== nextTargetId) {
      setAssignmentForm((current) => ({
        ...current,
        targetId: nextTargetId,
      }));
    }
  }, [
    assignmentForm.targetId,
    assignmentForm.targetKind,
    assignmentTargetOptions,
  ]);

  const runMutation = async (task: () => Promise<void>, success: string) => {
    setIsSaving(true);
    setError(null);
    setSuccessMessage(null);

    try {
      await task();
      await refreshInventory(false);
      setSuccessMessage(success);
    } catch (mutationError) {
      setError(
        mutationError instanceof Error ? mutationError.message : String(mutationError),
      );
    } finally {
      setIsSaving(false);
    }
  };

  const saveProvider = () => {
    void runMutation(async () => {
      await upsertSecretProvider(providerForm);
      setProviderForm(emptyProviderForm());
    }, providerForm.id ? "Provider updated." : "Provider created.");
  };

  const saveCredential = () => {
    void runMutation(async () => {
      await upsertSecretCredential({
        ...credentialForm,
        secretValue: credentialForm.secretValue?.trim() ? credentialForm.secretValue : undefined,
        clearSecret:
          credentialForm.clearSecret && !credentialForm.secretValue?.trim(),
      });
      setCredentialForm((current) => ({
        ...emptyCredentialForm(),
        providerId: inventory?.providers[0]?.id ?? current.providerId,
      }));
    }, credentialForm.id ? "Credential updated." : "Credential created.");
  };

  const saveAssignment = () => {
    void runMutation(async () => {
      await upsertSecretAssignment(assignmentForm);
      setAssignmentForm((current) => ({
        ...emptyAssignmentForm(),
        credentialId: inventory?.credentials[0]?.id ?? current.credentialId,
        targetKind: current.targetKind,
        targetId: assignmentTargetOptions[current.targetKind][0]?.id ?? "",
      }));
    }, assignmentForm.id ? "Assignment updated." : "Assignment created.");
  };

  const startEditingProvider = (provider: SecretProviderRecord) => {
    setProviderForm({
      id: provider.id,
      label: provider.label,
      providerKind: provider.providerKind,
      baseUrl: provider.baseUrl,
      description: provider.description,
    });
    setSuccessMessage(null);
    setError(null);
  };

  const startEditingCredential = (credential: SecretCredentialRecord) => {
    setCredentialForm({
      id: credential.id,
      providerId: credential.providerId,
      label: credential.label,
      description: credential.description,
      secretValue: "",
      clearSecret: false,
    });
    setSuccessMessage(null);
    setError(null);
  };

  const startEditingAssignment = (assignment: SecretAssignmentRecord) => {
    setAssignmentForm({
      id: assignment.id,
      credentialId: assignment.credentialId,
      targetKind: assignment.targetKind,
      targetId: assignment.targetId,
      envVar: assignment.envVar,
      filePath: assignment.filePath,
      enabled: assignment.enabled,
    });
    setSuccessMessage(null);
    setError(null);
  };

  const removeProvider = (providerId: string) => {
    void runMutation(async () => {
      await deleteSecretProvider(providerId);
      if (providerForm.id === providerId) {
        setProviderForm(emptyProviderForm());
      }
    }, "Provider deleted.");
  };

  const removeCredential = (credentialId: string) => {
    void runMutation(async () => {
      await deleteSecretCredential(credentialId);
      if (credentialForm.id === credentialId) {
        setCredentialForm((current) => ({
          ...emptyCredentialForm(),
          providerId: inventory?.providers[0]?.id ?? current.providerId,
        }));
      }
    }, "Credential deleted.");
  };

  const removeAssignment = (assignmentId: string) => {
    void runMutation(async () => {
      await deleteSecretAssignment(assignmentId);
      if (assignmentForm.id === assignmentId) {
        setAssignmentForm((current) => ({
          ...emptyAssignmentForm(),
          credentialId: inventory?.credentials[0]?.id ?? current.credentialId,
          targetKind: current.targetKind,
          targetId: assignmentTargetOptions[current.targetKind][0]?.id ?? "",
        }));
      }
    }, "Assignment deleted.");
  };

  const resetProviderForm = () => {
    setProviderForm(emptyProviderForm());
  };

  const resetCredentialForm = () => {
    setCredentialForm({
      ...emptyCredentialForm(),
      providerId: inventory?.providers[0]?.id ?? "",
    });
  };

  const resetAssignmentForm = () => {
    setAssignmentForm({
      ...emptyAssignmentForm(),
      credentialId: inventory?.credentials[0]?.id ?? "",
      targetKind: "machine",
      targetId: assignmentTargetOptions.machine[0]?.id ?? "",
    });
  };

  return (
    <section className="settings-section">
      <div className="settings-section-heading">
        <h3 className="settings-section-title">Secrets</h3>
        <div className="settings-option-group">
          <Button
            onClick={() => {
              void refreshInventory(true);
            }}
            size="sm"
            variant="outline"
          >
            {isLoading ? "Refreshing…" : "Refresh"}
          </Button>
        </div>
      </div>
      <p className="settings-editor-hint">
        Provider, credential, and assignment metadata live in SQLite. Secret values stay in the
        OS credential store and cannot be read back into the UI after save.
      </p>
      {error ? <p className="settings-error">{error}</p> : null}
      {successMessage ? <p className="settings-success">{successMessage}</p> : null}
      <section className="settings-runtime-card">
        <div className="settings-runtime-header">
          <div className="settings-runtime-copy">
            <span className="settings-runtime-label">Secret Store</span>
            <span className="settings-runtime-key">
              {inventory?.dbPath || "Secret database unavailable"}
            </span>
          </div>
        </div>
        <div className="settings-runtime-grid">
          <div className="settings-runtime-row">
            <strong>Backend</strong>
            {inventory?.storeBackend ?? "unknown"}
          </div>
          <div className="settings-runtime-row">
            <strong>Providers</strong>
            {inventory?.providers.length ?? 0} · <strong>Credentials</strong>{" "}
            {inventory?.credentials.length ?? 0} · <strong>Assignments</strong>{" "}
            {inventory?.assignments.length ?? 0}
          </div>
          <div className="settings-runtime-row">
            <strong>Audit events</strong>
            {inventory?.auditEvents.length ?? 0}
          </div>
        </div>
      </section>

      <Tabs
        className="settings-secret-tabs"
        onValueChange={(value) => setActiveTab(value as SecretSettingsTab)}
        value={activeTab}
      >
        <TabsList className="settings-tab-bar settings-secret-tab-bar" aria-label="Secret sections">
          <TabsTrigger className="settings-tab" value="providers">
            Providers
          </TabsTrigger>
          <TabsTrigger className="settings-tab" value="credentials">
            Credentials
          </TabsTrigger>
          <TabsTrigger className="settings-tab" value="assignments">
            Assignments
          </TabsTrigger>
          <TabsTrigger className="settings-tab" value="audit">
            Audit
          </TabsTrigger>
        </TabsList>

        <TabsContent className="settings-tab-panel settings-secret-tab-panel" value="providers">
          <section className="settings-runtime-card">
          <div className="settings-runtime-header">
            <div className="settings-runtime-copy">
              <span className="settings-runtime-label">Providers</span>
              <span className="settings-runtime-key">
                {inventory?.providers.length ?? 0} configured
              </span>
            </div>
          </div>
          <div className="settings-secret-form-grid">
            <label className="settings-field">
              <span className="settings-label">Label</span>
              <input
                className="dialog-input"
                onChange={(event) =>
                  setProviderForm((current) => ({
                    ...current,
                    label: event.target.value,
                  }))
                }
                placeholder="OpenAI production"
                value={providerForm.label}
              />
            </label>
            <label className="settings-field">
              <span className="settings-label">Provider kind</span>
              <select
                className="dialog-input"
                onChange={(event) =>
                  setProviderForm((current) => ({
                    ...current,
                    providerKind: event.target.value,
                  }))
                }
                value={providerForm.providerKind}
              >
                {PROVIDER_KIND_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="settings-field settings-secret-form-grid-full">
              <span className="settings-label">Base URL</span>
              <input
                className="dialog-input dialog-input-mono"
                onChange={(event) =>
                  setProviderForm((current) => ({
                    ...current,
                    baseUrl: event.target.value,
                  }))
                }
                placeholder="https://api.openai.com/v1"
                value={providerForm.baseUrl}
              />
            </label>
            <label className="settings-field settings-secret-form-grid-full">
              <span className="settings-label">Description</span>
              <textarea
                className="dialog-input dialog-input-multiline"
                onChange={(event) =>
                  setProviderForm((current) => ({
                    ...current,
                    description: event.target.value,
                  }))
                }
                placeholder="Primary provider for shared LLM usage."
                value={providerForm.description}
              />
            </label>
          </div>
          <div className="settings-secret-actions">
            <Button
              disabled={isSaving || !providerForm.label.trim()}
              onClick={saveProvider}
              size="sm"
            >
              {providerForm.id ? "Save provider" : "Add provider"}
            </Button>
            <Button
              disabled={isSaving}
              onClick={resetProviderForm}
              size="sm"
              variant="ghost"
            >
              Reset
            </Button>
          </div>
          <div className="settings-secret-list">
            {(inventory?.providers ?? []).map((provider) => (
              <div className="settings-secret-item" key={provider.id}>
                <div className="settings-secret-copy">
                  <div className="settings-secret-title-row">
                    <span className="settings-server-label">{provider.label}</span>
                    <Badge variant="neutral">{provider.providerKind}</Badge>
                  </div>
                  <span className="settings-server-host">
                    {provider.baseUrl || "Default endpoint"}
                  </span>
                  {provider.description ? (
                    <span className="settings-server-desc">{provider.description}</span>
                  ) : null}
                </div>
                <div className="settings-secret-actions">
                  <Button
                    onClick={() => startEditingProvider(provider)}
                    size="sm"
                    variant="outline"
                  >
                    Edit
                  </Button>
                  <Button
                    onClick={() => removeProvider(provider.id)}
                    size="sm"
                    variant="ghost"
                  >
                    Delete
                  </Button>
                </div>
              </div>
            ))}
            {(inventory?.providers.length ?? 0) === 0 ? (
              <p className="settings-empty">No providers configured.</p>
            ) : null}
          </div>
          </section>
        </TabsContent>

        <TabsContent className="settings-tab-panel settings-secret-tab-panel" value="credentials">
          <section className="settings-runtime-card">
          <div className="settings-runtime-header">
            <div className="settings-runtime-copy">
              <span className="settings-runtime-label">Credentials</span>
              <span className="settings-runtime-key">
                {inventory?.credentials.length ?? 0} stored references
              </span>
            </div>
          </div>
          <div className="settings-secret-form-grid">
            <label className="settings-field">
              <span className="settings-label">Provider</span>
              <select
                className="dialog-input"
                onChange={(event) =>
                  setCredentialForm((current) => ({
                    ...current,
                    providerId: event.target.value,
                  }))
                }
                value={credentialForm.providerId}
              >
                <option value="">Select a provider</option>
                {(inventory?.providers ?? []).map((provider) => (
                  <option key={provider.id} value={provider.id}>
                    {provider.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="settings-field">
              <span className="settings-label">Label</span>
              <input
                className="dialog-input"
                onChange={(event) =>
                  setCredentialForm((current) => ({
                    ...current,
                    label: event.target.value,
                  }))
                }
                placeholder="shared-api-key"
                value={credentialForm.label}
              />
            </label>
            <label className="settings-field settings-secret-form-grid-full">
              <span className="settings-label">Description</span>
              <input
                className="dialog-input"
                onChange={(event) =>
                  setCredentialForm((current) => ({
                    ...current,
                    description: event.target.value,
                  }))
                }
                placeholder="Used by orchestration workloads."
                value={credentialForm.description}
              />
            </label>
            <label className="settings-field settings-secret-form-grid-full">
              <span className="settings-label">
                Secret value
                {credentialForm.id ? " (leave blank to keep current value)" : ""}
              </span>
              <input
                className="dialog-input dialog-input-mono"
                onChange={(event) =>
                  setCredentialForm((current) => ({
                    ...current,
                    secretValue: event.target.value,
                    clearSecret: event.target.value.trim() ? false : current.clearSecret,
                  }))
                }
                placeholder="sk-..."
                type="password"
                value={credentialForm.secretValue ?? ""}
              />
            </label>
            {credentialForm.id ? (
              <label className="settings-secret-checkbox">
                <input
                  checked={Boolean(credentialForm.clearSecret)}
                  onChange={(event) =>
                    setCredentialForm((current) => ({
                      ...current,
                      clearSecret: event.target.checked,
                      secretValue: event.target.checked ? "" : current.secretValue,
                    }))
                  }
                  type="checkbox"
                />
                Clear stored secret on next save
              </label>
            ) : null}
          </div>
          <div className="settings-secret-actions">
            <Button
              disabled={isSaving || !credentialForm.providerId || !credentialForm.label.trim()}
              onClick={saveCredential}
              size="sm"
            >
              {credentialForm.id ? "Save credential" : "Add credential"}
            </Button>
            <Button
              disabled={isSaving}
              onClick={resetCredentialForm}
              size="sm"
              variant="ghost"
            >
              Reset
            </Button>
          </div>
          <div className="settings-secret-list">
            {(inventory?.credentials ?? []).map((credential) => {
              const provider = providerMap.get(credential.providerId);

              return (
                <div className="settings-secret-item" key={credential.id}>
                  <div className="settings-secret-copy">
                    <div className="settings-secret-title-row">
                      <span className="settings-server-label">{credential.label}</span>
                      {credential.hasSecret ? (
                        <Badge variant="success">Stored</Badge>
                      ) : (
                        <Badge variant="warning">Missing secret</Badge>
                      )}
                    </div>
                    <span className="settings-server-host">
                      {provider?.label ?? credential.providerId}
                    </span>
                    {credential.description ? (
                      <span className="settings-server-desc">{credential.description}</span>
                    ) : null}
                    <span className="settings-secret-meta">
                      {credential.secretBackend} · rotated{" "}
                      {formatTimestamp(credential.lastRotatedAtMs)}
                    </span>
                  </div>
                  <div className="settings-secret-actions">
                    <Button
                      onClick={() => startEditingCredential(credential)}
                      size="sm"
                      variant="outline"
                    >
                      Edit
                    </Button>
                    <Button
                      onClick={() => removeCredential(credential.id)}
                      size="sm"
                      variant="ghost"
                    >
                      Delete
                    </Button>
                  </div>
                </div>
              );
            })}
            {(inventory?.credentials.length ?? 0) === 0 ? (
              <p className="settings-empty">No credentials configured.</p>
            ) : null}
          </div>
          </section>
        </TabsContent>

        <TabsContent className="settings-tab-panel settings-secret-tab-panel" value="assignments">
          <section className="settings-runtime-card">
          <div className="settings-runtime-header">
            <div className="settings-runtime-copy">
              <span className="settings-runtime-label">Assignments</span>
              <span className="settings-runtime-key">
                {inventory?.assignments.length ?? 0} delivery rules
              </span>
            </div>
          </div>
          <div className="settings-secret-form-grid">
            <label className="settings-field">
              <span className="settings-label">Credential</span>
              <select
                className="dialog-input"
                onChange={(event) =>
                  setAssignmentForm((current) => ({
                    ...current,
                    credentialId: event.target.value,
                  }))
                }
                value={assignmentForm.credentialId}
              >
                <option value="">Select a credential</option>
                {(inventory?.credentials ?? []).map((credential) => (
                  <option key={credential.id} value={credential.id}>
                    {credential.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="settings-field">
              <span className="settings-label">Target kind</span>
              <select
                className="dialog-input"
                onChange={(event) => {
                  const nextTargetKind = event.target.value as SecretAssignmentTargetKind;
                  setAssignmentForm((current) => ({
                    ...current,
                    targetKind: nextTargetKind,
                    targetId: assignmentTargetOptions[nextTargetKind][0]?.id ?? "",
                  }));
                }}
                value={assignmentForm.targetKind}
              >
                <option value="machine">Machine</option>
                <option value="container">Container</option>
              </select>
            </label>
            <label className="settings-field settings-secret-form-grid-full">
              <span className="settings-label">Target</span>
              <select
                className="dialog-input"
                onChange={(event) =>
                  setAssignmentForm((current) => ({
                    ...current,
                    targetId: event.target.value,
                  }))
                }
                value={assignmentForm.targetId}
              >
                <option value="">Select a target</option>
                {assignmentTargetOptions[assignmentForm.targetKind].map((target) => (
                  <option key={target.id} value={target.id}>
                    {target.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="settings-field">
              <span className="settings-label">Environment variable</span>
              <input
                className="dialog-input dialog-input-mono"
                onChange={(event) =>
                  setAssignmentForm((current) => ({
                    ...current,
                    envVar: event.target.value,
                  }))
                }
                placeholder="OPENAI_API_KEY"
                value={assignmentForm.envVar}
              />
            </label>
            <label className="settings-field">
              <span className="settings-label">File path</span>
              <input
                className="dialog-input dialog-input-mono"
                onChange={(event) =>
                  setAssignmentForm((current) => ({
                    ...current,
                    filePath: event.target.value,
                  }))
                }
                placeholder="~/.config/ark/openai.key"
                value={assignmentForm.filePath}
              />
            </label>
            <label className="settings-secret-checkbox settings-secret-form-grid-full">
              <input
                checked={Boolean(assignmentForm.enabled)}
                onChange={(event) =>
                  setAssignmentForm((current) => ({
                    ...current,
                    enabled: event.target.checked,
                  }))
                }
                type="checkbox"
              />
              Assignment enabled
            </label>
          </div>
          <div className="settings-secret-actions">
            <Button
              disabled={
                isSaving ||
                !assignmentForm.credentialId ||
                !assignmentForm.targetId ||
                (!assignmentForm.envVar?.trim() && !assignmentForm.filePath?.trim())
              }
              onClick={saveAssignment}
              size="sm"
            >
              {assignmentForm.id ? "Save assignment" : "Add assignment"}
            </Button>
            <Button
              disabled={isSaving}
              onClick={resetAssignmentForm}
              size="sm"
              variant="ghost"
            >
              Reset
            </Button>
          </div>
          <div className="settings-secret-list">
            {(inventory?.assignments ?? []).map((assignment) => {
              const credential = credentialMap.get(assignment.credentialId);
              const targetLabel =
                targetLabelMap.get(assignment.targetId) ?? assignment.targetId;

              return (
                <div className="settings-secret-item" key={assignment.id}>
                  <div className="settings-secret-copy">
                    <div className="settings-secret-title-row">
                      <span className="settings-server-label">
                        {credential?.label ?? assignment.credentialId}
                      </span>
                      <Badge variant={assignment.enabled ? "success" : "neutral"}>
                        {assignment.targetKind}
                      </Badge>
                    </div>
                    <span className="settings-server-host">{targetLabel}</span>
                    <span className="settings-secret-meta">
                      {assignment.envVar || "no env var"}
                      {assignment.filePath ? ` · ${assignment.filePath}` : ""}
                    </span>
                  </div>
                  <div className="settings-secret-actions">
                    <Button
                      onClick={() => startEditingAssignment(assignment)}
                      size="sm"
                      variant="outline"
                    >
                      Edit
                    </Button>
                    <Button
                      onClick={() => removeAssignment(assignment.id)}
                      size="sm"
                      variant="ghost"
                    >
                      Delete
                    </Button>
                  </div>
                </div>
              );
            })}
            {(inventory?.assignments.length ?? 0) === 0 ? (
              <p className="settings-empty">No assignments configured.</p>
            ) : null}
          </div>
          </section>
        </TabsContent>

        <TabsContent className="settings-tab-panel settings-secret-tab-panel" value="audit">
          <section className="settings-runtime-card">
            <div className="settings-runtime-header">
              <div className="settings-runtime-copy">
                <span className="settings-runtime-label">Audit</span>
                <span className="settings-runtime-key">
                  Latest secret-management mutations
                </span>
              </div>
            </div>
            <div className="settings-secret-list">
              {(inventory?.auditEvents ?? []).map((event) => (
                <div className="settings-secret-item" key={`${event.id}`}>
                  <div className="settings-secret-copy">
                    <div className="settings-secret-title-row">
                      <span className="settings-server-label">{event.eventKind}</span>
                      <Badge variant="neutral">{event.entityKind}</Badge>
                    </div>
                    <span className="settings-secret-meta">
                      {event.entityId} · {formatTimestamp(event.createdAtMs)}
                    </span>
                    {event.detail ? (
                      <span className="settings-server-desc">{event.detail}</span>
                    ) : null}
                  </div>
                </div>
              ))}
              {(inventory?.auditEvents.length ?? 0) === 0 ? (
                <p className="settings-empty">No audit events recorded yet.</p>
              ) : null}
            </div>
          </section>
        </TabsContent>
      </Tabs>
    </section>
  );
}
