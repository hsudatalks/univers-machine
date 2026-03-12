import { Cloud, Network, Server } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { MachineImportCandidate } from "../types";
import {
  loadTargetsConfig,
  scanSshConfigMachineCandidates,
  scanTailscaleMachineCandidates,
  updateTargetsConfig,
} from "../lib/tauri";
import {
  createEmptyMachine,
  parseTargetsConfig,
  stringifyTargetsConfig,
  type MachineConfig,
} from "../lib/targets-config";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";

type MachineImportSource = "ssh-config" | "tailscale" | "custom";
type ProviderKind = "machine" | "cloud-vm" | "kubernetes";

interface AddMachineDialogProps {
  onClose: () => void;
  onImported: () => void;
  onOpenCustom: () => void;
}

function nextMachineId(baseId: string, existingIds: Set<string>): string {
  const normalizedBase = baseId.trim() || "machine";

  if (!existingIds.has(normalizedBase)) {
    existingIds.add(normalizedBase);
    return normalizedBase;
  }

  let suffix = 2;
  while (existingIds.has(`${normalizedBase}-${suffix}`)) {
    suffix += 1;
  }

  const machineId = `${normalizedBase}-${suffix}`;
  existingIds.add(machineId);
  return machineId;
}

function importedCandidateToMachine(
  candidate: MachineImportCandidate,
  profileId: string,
  existingIds: Set<string>,
): MachineConfig {
  const machine = createEmptyMachine(profileId);

  return {
    ...machine,
    id: nextMachineId(candidate.machineId, existingIds),
    label: candidate.label || candidate.machineId,
    transport: "ssh",
    host: candidate.host,
    port: candidate.port,
    description: candidate.description,
    sshUser: candidate.sshUser || machine.sshUser,
    identityFiles: candidate.identityFiles,
    jumpChain: candidate.jumpChain,
  };
}

function sourceLabel(source: MachineImportSource): string {
  switch (source) {
    case "ssh-config":
      return "SSH Config";
    case "tailscale":
      return "Tailscale";
    case "custom":
      return "Manual";
  }
}

export function AddMachineDialog({
  onClose,
  onImported,
  onOpenCustom,
}: AddMachineDialogProps) {
  const activeProviderKind: ProviderKind = "machine";
  const [activeSource, setActiveSource] = useState<MachineImportSource>("ssh-config");
  const [scanError, setScanError] = useState<string | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [candidatesBySource, setCandidatesBySource] = useState<
    Partial<Record<Exclude<MachineImportSource, "custom">, MachineImportCandidate[]>>
  >({});
  const [selectedImportIdsBySource, setSelectedImportIdsBySource] = useState<
    Partial<Record<Exclude<MachineImportSource, "custom">, string[]>>
  >({});

  const candidates =
    activeSource === "custom" ? [] : candidatesBySource[activeSource] ?? [];
  const selectedImportIds =
    activeSource === "custom" ? [] : selectedImportIdsBySource[activeSource] ?? [];
  const selectedCandidates = useMemo(
    () => candidates.filter((candidate) => selectedImportIds.includes(candidate.importId)),
    [candidates, selectedImportIds],
  );

  const handleScan = async (source = activeSource) => {
    if (source === "custom") {
      return;
    }

    setIsScanning(true);
    setScanError(null);

    try {
      const nextCandidates =
        source === "ssh-config"
          ? await scanSshConfigMachineCandidates()
          : await scanTailscaleMachineCandidates();

      setCandidatesBySource((current) => ({ ...current, [source]: nextCandidates }));
      setSelectedImportIdsBySource((current) => ({
        ...current,
        [source]:
          source === "ssh-config"
            ? []
            : nextCandidates.map((candidate) => candidate.importId),
      }));
    } catch (error) {
      setCandidatesBySource((current) => ({ ...current, [source]: [] }));
      setSelectedImportIdsBySource((current) => ({ ...current, [source]: [] }));
      setScanError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsScanning(false);
    }
  };

  useEffect(() => {
    if (activeSource === "custom" || candidatesBySource[activeSource]) {
      return;
    }

    void handleScan(activeSource);
  }, [activeSource]); // eslint-disable-line react-hooks/exhaustive-deps

  const toggleCandidate = (importId: string) => {
    if (activeSource === "custom") {
      return;
    }

    setSelectedImportIdsBySource((current) => {
      const selected = current[activeSource] ?? [];
      return {
        ...current,
        [activeSource]: selected.includes(importId)
          ? selected.filter((currentId) => currentId !== importId)
          : [...selected, importId],
      };
    });
  };

  const setSelectedForActiveSource = (nextImportIds: string[]) => {
    if (activeSource === "custom") {
      return;
    }

    setSelectedImportIdsBySource((current) => ({
      ...current,
      [activeSource]: nextImportIds,
    }));
  };

  const handleImport = async () => {
    if (activeSource === "custom" || selectedCandidates.length === 0) {
      return;
    }

    setIsImporting(true);
    setScanError(null);

    try {
      const raw = await loadTargetsConfig();
      const parsed = parseTargetsConfig(raw);
      const profileId = parsed.defaultProfile ?? Object.keys(parsed.profiles)[0] ?? "";
      const existingIds = new Set(parsed.machines.map((machine) => machine.id));

      parsed.machines = [
        ...parsed.machines,
        ...selectedCandidates.map((candidate) =>
          importedCandidateToMachine(candidate, profileId, existingIds),
        ),
      ];

      await updateTargetsConfig(stringifyTargetsConfig(parsed));
      onImported();
      onClose();
    } catch (error) {
      setScanError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsImporting(false);
    }
  };

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div
        aria-label="Add provider"
        className="dialog-panel dialog-panel-wide"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
      >
        <header className="dialog-header">
          <div className="dialog-header-copy">
            <span className="dialog-title">Add provider</span>
            <span className="dialog-subtitle">Providers supply workbenches. Choose a provider type, then select its setup path.</span>
          </div>
          <Button aria-label="Close" className="dialog-close" onClick={onClose} size="icon" variant="ghost">
            <svg aria-hidden="true" className="panel-button-icon-svg" fill="none" viewBox="0 0 16 16">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeLinecap="round" strokeWidth="1.4" />
            </svg>
          </Button>
        </header>

        <div className="dialog-body add-provider-body">
          <div className="dialog-card add-provider-hero">
            <div className="dialog-card-header">
              <div>
                <div className="dialog-card-title">Provider types</div>
                <p className="dialog-empty">
                  Providers define where workbenches come from. Machine providers are available today.
                </p>
              </div>
              <Badge variant="neutral">Expandable</Badge>
            </div>

            <div className="provider-type-grid">
              <button
                aria-pressed={activeProviderKind === "machine"}
                className="provider-type-card is-active"
                type="button"
              >
                <div className="provider-type-card-head">
                  <span className="provider-type-icon">
                    <Server size={16} />
                  </span>
                  <Badge variant="neutral">Available now</Badge>
                </div>
                <div className="provider-type-card-title">Machine</div>
                <p className="dialog-empty">
                  Connect to an SSH-managed host, then discover and manage the container workbenches that live on it.
                </p>
              </button>

              <button
                aria-disabled="true"
                className="provider-type-card is-disabled"
                type="button"
              >
                <div className="provider-type-card-head">
                  <span className="provider-type-icon">
                    <Cloud size={16} />
                  </span>
                  <Badge variant="neutral">Soon</Badge>
                </div>
                <div className="provider-type-card-title">Cloud VM</div>
                <p className="dialog-empty">
                  Inventory and connect virtual machines from cloud accounts like AWS, Azure, and GCP.
                </p>
              </button>

              <button
                aria-disabled="true"
                className="provider-type-card is-disabled"
                type="button"
              >
                <div className="provider-type-card-head">
                  <span className="provider-type-icon">
                    <Network size={16} />
                  </span>
                  <Badge variant="neutral">Soon</Badge>
                </div>
                <div className="provider-type-card-title">Kubernetes</div>
                <p className="dialog-empty">
                  Surface pods and namespace workspaces as first-class workbenches from cluster providers.
                </p>
              </button>
            </div>
          </div>

          <div className="dialog-card">
            <div className="dialog-card-header">
              <div>
                <div className="dialog-card-title">Machine setup</div>
                <p className="dialog-empty">
                  Pick how this machine provider host should be discovered or defined.
                </p>
              </div>
              <Badge variant="neutral">3 paths</Badge>
            </div>

            <div className="provider-source-grid">
              <button
                className={`provider-source-card${activeSource === "ssh-config" ? " is-active" : ""}`}
                onClick={() => setActiveSource("ssh-config")}
                type="button"
              >
                <div className="provider-source-card-head">
                  <div className="provider-source-card-title">SSH Config</div>
                  <Badge variant="neutral">Bulk import</Badge>
                </div>
                <p className="dialog-empty">
                  Scan named hosts and jump chains from your local SSH config, then import the provider hosts you want.
                </p>
              </button>

              <button
                className={`provider-source-card${activeSource === "tailscale" ? " is-active" : ""}`}
                onClick={() => setActiveSource("tailscale")}
                type="button"
              >
                <div className="provider-source-card-head">
                  <div className="provider-source-card-title">Tailscale</div>
                  <Badge variant="neutral">Tailnet discovery</Badge>
                </div>
                <p className="dialog-empty">
                  Discover reachable tailnet machines and turn the ones you want into managed provider hosts.
                </p>
              </button>

              <button
                className={`provider-source-card${activeSource === "custom" ? " is-active" : ""}`}
                onClick={() => setActiveSource("custom")}
                type="button"
              >
                <div className="provider-source-card-head">
                  <div className="provider-source-card-title">Manual</div>
                  <Badge variant="neutral">Full control</Badge>
                </div>
                <p className="dialog-empty">
                  Start from a blank provider host and define the SSH identity, discovery behavior, and defaults.
                </p>
              </button>
            </div>
          </div>

          {activeSource === "ssh-config" ? (
            <MachineImportTab
              candidates={candidates}
              error={scanError}
              importLabel="Import selected providers"
              isImporting={isImporting}
              isScanning={isScanning}
              onImport={handleImport}
              onClearSelection={() => setSelectedForActiveSource([])}
              onRescan={() => void handleScan("ssh-config")}
              onSelectAll={() => setSelectedForActiveSource(candidates.map((candidate) => candidate.importId))}
              onToggleCandidate={toggleCandidate}
              selectedImportIds={selectedImportIds}
              sourceLabel={sourceLabel("ssh-config")}
            />
          ) : null}

          {activeSource === "tailscale" ? (
            <MachineImportTab
              candidates={candidates}
              error={scanError}
              importLabel="Import selected providers"
              isImporting={isImporting}
              isScanning={isScanning}
              onImport={handleImport}
              onClearSelection={() => setSelectedForActiveSource([])}
              onRescan={() => void handleScan("tailscale")}
              onSelectAll={() => setSelectedForActiveSource(candidates.map((candidate) => candidate.importId))}
              onToggleCandidate={toggleCandidate}
              selectedImportIds={selectedImportIds}
              sourceLabel={sourceLabel("tailscale")}
            />
          ) : null}

          {activeSource === "custom" ? (
            <div className="dialog-tab-content">
              <div className="dialog-card">
                <div className="dialog-card-header">
                  <div>
                    <div className="dialog-card-title">Manual machine provider</div>
                    <p className="dialog-empty">
                      Define the SSH connection, discovery mode, and provider defaults yourself.
                    </p>
                  </div>
                  <Badge variant="neutral">Manual</Badge>
                </div>

                <div className="dialog-field-grid">
                  <div className="dialog-field">
                    <span className="dialog-field-label">Use when</span>
                    <span className="dialog-field-value">
                      You want full control over the provider host, identity files, discovery mode, and defaults.
                    </span>
                  </div>
                </div>
              </div>

              <div className="dialog-section-actions">
                <Button
                  onClick={() => {
                    onClose();
                    onOpenCustom();
                  }}
                >
                  Open manual setup
                </Button>
              </div>
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function MachineImportTab({
  candidates,
  error,
  importLabel,
  isImporting,
  isScanning,
  onClearSelection,
  onImport,
  onRescan,
  onSelectAll,
  onToggleCandidate,
  selectedImportIds,
  sourceLabel,
}: {
  candidates: MachineImportCandidate[];
  error: string | null;
  importLabel: string;
  isImporting: boolean;
  isScanning: boolean;
  onClearSelection: () => void;
  onImport: () => void;
  onRescan: () => void;
  onSelectAll: () => void;
  onToggleCandidate: (importId: string) => void;
  selectedImportIds: string[];
  sourceLabel: string;
}) {
  return (
    <div className="dialog-tab-content">
      <div className="dialog-card">
        <div className="dialog-card-header">
          <div>
            <div className="dialog-card-title">{sourceLabel} scan</div>
            <p className="dialog-empty">
              Scan available provider hosts, review the candidates, and import the ones you want.
            </p>
          </div>
          <Badge variant="neutral">{candidates.length} found</Badge>
        </div>

        <div className="dialog-inline-actions">
          <Button disabled={isScanning} onClick={onRescan} size="sm" variant="outline">
            {isScanning ? "Scanning…" : `Scan ${sourceLabel}`}
          </Button>
          <Button
            disabled={candidates.length === 0 || isScanning}
            onClick={onSelectAll}
            size="sm"
            variant="ghost"
          >
            Select all
          </Button>
          <Button
            disabled={selectedImportIds.length === 0 || isScanning}
            onClick={onClearSelection}
            size="sm"
            variant="ghost"
          >
            Clear
          </Button>
          <Button
            disabled={isImporting || selectedImportIds.length === 0}
            onClick={onImport}
            size="sm"
          >
            {isImporting ? "Importing…" : `${importLabel} (${selectedImportIds.length})`}
          </Button>
        </div>

        {error ? <p className="dialog-error">{error}</p> : null}
      </div>

      {candidates.length === 0 && !error ? (
        <p className="dialog-empty">No provider host candidates found yet.</p>
      ) : null}

      <div className="dialog-choice-list">
        {candidates.map((candidate) => {
          const isSelected = selectedImportIds.includes(candidate.importId);

          return (
            <label className="dialog-choice-card" key={candidate.importId}>
              <input
                checked={isSelected}
                className="dialog-choice-checkbox"
                onChange={() => onToggleCandidate(candidate.importId)}
                type="checkbox"
              />

              <div className="dialog-choice-copy">
                <div className="dialog-choice-title-row">
                  <div>
                    <div className="dialog-card-title">{candidate.label}</div>
                    <div className="dialog-field-mono">{candidate.machineId}</div>
                  </div>
                  <Badge variant="neutral">Machine provider</Badge>
                </div>

                <div className="dialog-choice-meta">
                  <span className="dialog-chip">{candidate.host}</span>
                  <span className="dialog-chip">{candidate.sshUser}</span>
                  <span className="dialog-chip">port {candidate.port}</span>
                  {candidate.jumpChain.length > 0 ? (
                    <span className="dialog-chip">{candidate.jumpChain.length} jump hop(s)</span>
                  ) : null}
                </div>

                <p className="dialog-empty">{candidate.description}</p>
                <div className="dialog-field-mono">{candidate.detail}</div>
              </div>
            </label>
          );
        })}
      </div>
    </div>
  );
}
