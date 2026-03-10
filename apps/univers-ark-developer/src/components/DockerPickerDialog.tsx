import { useEffect, useState } from "react";
import type { LocalDockerContainer } from "../types";
import { scanLocalContainers, loadTargetsConfig, updateTargetsConfig } from "../lib/tauri";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";

interface DockerPickerDialogProps {
  onClose: () => void;
  onSaved: () => void;
}

function statusVariant(status: string): "success" | "warning" | "destructive" | "neutral" {
  if (status === "running") return "success";
  if (status === "paused") return "warning";
  if (status === "exited" || status === "stopped") return "destructive";
  return "neutral";
}

export function DockerPickerDialog({ onClose, onSaved }: DockerPickerDialogProps) {
  const [containers, setContainers] = useState<LocalDockerContainer[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [alreadyAdded, setAlreadyAdded] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");

  useEffect(() => {
    void (async () => {
      try {
        const [scanned, raw] = await Promise.all([
          scanLocalContainers(),
          loadTargetsConfig(),
        ]);
        const config = JSON.parse(raw);
        const existingIds = new Set<string>(
          (config.targets ?? []).map((t: { id: string }) => t.id)
        );
        const added = new Set<string>();
        for (const c of scanned) {
          if (existingIds.has(`${c.runtime}-${c.name}`)) added.add(`${c.runtime}-${c.name}`);
        }
        setContainers(scanned);
        setAlreadyAdded(added);
      } catch (err) {
        setError(String(err));
      } finally {
        setIsLoading(false);
      }
    })();
  }, []);

  const toggle = (key: string) => {
    if (alreadyAdded.has(key)) return;
    setSelected((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });
  };

  const handleAdd = async () => {
    if (selected.size === 0) return;
    setIsSaving(true);
    setError(null);
    try {
      const raw = await loadTargetsConfig();
      const config = JSON.parse(raw);
      config.targets = config.targets ?? [];

      for (const c of containers) {
        const key = `${c.runtime}-${c.name}`;
        if (!selected.has(key)) continue;

        const surfaces = c.mappedPorts
          .filter((p) => p.containerPort !== 22 && p.protocol === "tcp")
          .map((p) => ({
            id: `port-${p.containerPort}`,
            label: `Port ${p.containerPort}`,
            tunnelCommand: "",
            localUrl: `http://localhost:${p.hostPort}/`,
            remoteUrl: `http://localhost:${p.containerPort}/`,
          }));

        config.targets.push({
          id: key,
          label: c.name,
          host: "localhost",
          description: c.description || `${c.runtime} container: ${c.name}${c.role ? ` (${c.role})` : ""}. Image: ${c.image}.`,
          terminalCommand: `${c.runtime} exec -it ${c.name} /bin/bash`,
          notes: [
            `${c.runtime} container. Image: ${c.image}.`,
            `Start/stop: ${c.runtime} start ${c.name} / ${c.runtime} stop ${c.name}`,
          ],
          surfaces,
        });
      }

      await updateTargetsConfig(JSON.stringify(config, null, 2));
      onSaved();
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSaving(false);
    }
  };

  const visible = containers.filter((c) => {
    if (!filter) return true;
    const q = filter.toLowerCase();
    return (
      c.name.toLowerCase().includes(q) ||
      c.role.toLowerCase().includes(q) ||
      c.image.toLowerCase().includes(q) ||
      c.runtime.toLowerCase().includes(q)
    );
  });

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div
        aria-label="Add local containers"
        className="dialog-panel docker-picker-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        style={{ width: 640, maxHeight: "80vh", display: "flex", flexDirection: "column" }}
      >
        {/* Header */}
        <header className="dialog-header">
          <div className="dialog-header-copy">
            <span className="dialog-title">Add local containers</span>
            <span className="dialog-subtitle">
              Scanning for Docker, Podman, and containerd (nerdctl) containers on this machine
            </span>
          </div>
          <Button aria-label="Close" className="dialog-close" onClick={onClose} size="icon" variant="ghost">
            <svg aria-hidden="true" className="panel-button-icon-svg" fill="none" viewBox="0 0 16 16">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeLinecap="round" strokeWidth="1.4" />
            </svg>
          </Button>
        </header>

        {/* Filter */}
        <div style={{ padding: "8px 20px 4px" }}>
          <input
            className="dialog-field-input"
            placeholder="Filter by name, role, or image…"
            style={{ width: "100%", boxSizing: "border-box" }}
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>

        {/* Container list */}
        <div className="dialog-body" style={{ flex: 1, overflowY: "auto", padding: "8px 20px" }}>
          {isLoading ? (
            <p className="settings-empty">Scanning for local containers…</p>
          ) : error ? (
            <p className="dialog-error">{error}</p>
          ) : visible.length === 0 ? (
            <p className="settings-empty">No containers found.</p>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              {visible.map((c) => {
                const key = `${c.runtime}-${c.name}`;
                const isAdded = alreadyAdded.has(key);
                const isSelected = selected.has(key);
                return (
                  <button
                    className={`settings-server-card${isSelected ? " is-selected" : ""}${isAdded ? " is-disabled" : ""}`}
                    key={key}
                    onClick={() => toggle(key)}
                    style={{ textAlign: "left", cursor: isAdded ? "default" : "pointer", opacity: isAdded ? 0.5 : 1 }}
                    type="button"
                  >
                    <div className="settings-server-header">
                      <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <span style={{ fontSize: 13, opacity: 0.5 }}>
                          {isAdded ? "✓" : isSelected ? "☑" : "☐"}
                        </span>
                        <span className="settings-server-label">{c.name}</span>
                        {c.role ? <span style={{ fontSize: 11, opacity: 0.6 }}>[{c.role}]</span> : null}
                      </span>
                      <div style={{ display: "flex", gap: 6 }}>
                        <Badge variant="neutral">{c.runtime}</Badge>
                        {isAdded && <Badge variant="neutral">Already added</Badge>}
                        <Badge variant={statusVariant(c.status)}>{c.status}</Badge>
                      </div>
                    </div>
                    <div className="settings-server-meta">
                      <span className="settings-server-host">{c.image}</span>
                      {c.description ? (
                        <span className="settings-server-desc">{c.description}</span>
                      ) : null}
                    </div>
                    {c.mappedPorts.length > 0 && (
                      <div className="settings-server-footer">
                        <span className="settings-server-containers">
                          Ports:{" "}
                          {c.mappedPorts
                            .map((p) => `${p.hostPort}→${p.containerPort}/${p.protocol}`)
                            .join(", ")}
                        </span>
                      </div>
                    )}
                  </button>
                );
              })}
            </div>
          )}
        </div>

        {/* Footer */}
        <footer className="dialog-footer">
          <Button
            disabled={isSaving || selected.size === 0}
            onClick={() => void handleAdd()}
            variant="default"
          >
            {isSaving ? "Adding…" : `Add ${selected.size > 0 ? `${selected.size} ` : ""}container${selected.size !== 1 ? "s" : ""}`}
          </Button>
          <Button onClick={onClose} variant="outline">Cancel</Button>
          <span style={{ marginRight: "auto", fontSize: 12, opacity: 0.6 }}>
            {containers.length} container{containers.length !== 1 ? "s" : ""} found
          </span>
        </footer>
      </div>
    </div>
  );
}
