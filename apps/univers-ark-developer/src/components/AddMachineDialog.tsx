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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";

type MachineImportSource = "ssh-config" | "tailscale" | "custom";

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
    managerType: "none",
    discoveryMode: "host-only",
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
      return "Custom";
  }
}

export function AddMachineDialog({
  onClose,
  onImported,
  onOpenCustom,
}: AddMachineDialogProps) {
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
        [source]: nextCandidates.map((candidate) => candidate.importId),
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
        aria-label="Add machine"
        className="dialog-panel"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
      >
        <header className="dialog-header">
          <div className="dialog-header-copy">
            <span className="dialog-title">Add machine</span>
            <span className="dialog-subtitle">Import from SSH config, Tailscale, or add one manually</span>
          </div>
          <Button aria-label="Close" className="dialog-close" onClick={onClose} size="icon" variant="ghost">
            <svg aria-hidden="true" className="panel-button-icon-svg" fill="none" viewBox="0 0 16 16">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeLinecap="round" strokeWidth="1.4" />
            </svg>
          </Button>
        </header>

        <Tabs onValueChange={(value) => setActiveSource(value as MachineImportSource)} value={activeSource}>
          <TabsList className="dialog-tabs" aria-label="Machine import sources">
            <TabsTrigger className="dialog-tab" value="ssh-config">SSH Config</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="tailscale">Tailscale</TabsTrigger>
            <TabsTrigger className="dialog-tab" value="custom">Custom</TabsTrigger>
          </TabsList>

          <div className="dialog-body">
            <TabsContent className="dialog-tab-panel" value="ssh-config">
              <MachineImportTab
                candidates={candidates}
                error={scanError}
                importLabel="Import selected machines"
                isImporting={isImporting}
                isScanning={isScanning}
                onImport={handleImport}
                onRescan={() => void handleScan("ssh-config")}
                onToggleCandidate={toggleCandidate}
                selectedImportIds={selectedImportIds}
                sourceLabel={sourceLabel("ssh-config")}
              />
            </TabsContent>

            <TabsContent className="dialog-tab-panel" value="tailscale">
              <MachineImportTab
                candidates={candidates}
                error={scanError}
                importLabel="Import selected machines"
                isImporting={isImporting}
                isScanning={isScanning}
                onImport={handleImport}
                onRescan={() => void handleScan("tailscale")}
                onToggleCandidate={toggleCandidate}
                selectedImportIds={selectedImportIds}
                sourceLabel={sourceLabel("tailscale")}
              />
            </TabsContent>

            <TabsContent className="dialog-tab-panel" value="custom">
              <div className="dialog-tab-content">
                <div className="dialog-card">
                  <div className="dialog-card-header">
                    <div>
                      <div className="dialog-card-title">Custom machine</div>
                      <p className="dialog-empty">
                        Define the SSH connection, discovery mode, and workspace defaults yourself.
                      </p>
                    </div>
                    <Badge variant="neutral">Manual</Badge>
                  </div>

                  <div className="dialog-field-grid">
                    <div className="dialog-field">
                      <span className="dialog-field-label">Use when</span>
                      <span className="dialog-field-value">
                        You want full control over host, user, identity files, discovery mode, and services.
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
            </TabsContent>
          </div>
        </Tabs>
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
  onImport,
  onRescan,
  onToggleCandidate,
  selectedImportIds,
  sourceLabel,
}: {
  candidates: MachineImportCandidate[];
  error: string | null;
  importLabel: string;
  isImporting: boolean;
  isScanning: boolean;
  onImport: () => void;
  onRescan: () => void;
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
              Scan available machine definitions, review the candidates, and import the ones you want.
            </p>
          </div>
          <Badge variant="neutral">{candidates.length} found</Badge>
        </div>

        <div className="dialog-inline-actions">
          <Button disabled={isScanning} onClick={onRescan} size="sm" variant="outline">
            {isScanning ? "Scanning…" : `Scan ${sourceLabel}`}
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
        <p className="dialog-empty">No machine candidates found yet.</p>
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
                  <Badge variant="neutral">Host only</Badge>
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
