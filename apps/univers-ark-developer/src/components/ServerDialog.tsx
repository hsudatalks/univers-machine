import { useState } from "react";
import type { ManagedServer } from "../types";

type ServerDialogTab = "general" | "containers";

interface ServerDialogProps {
  onClose: () => void;
  server: ManagedServer;
}

export function ServerDialog({ onClose, server }: ServerDialogProps) {
  const [activeTab, setActiveTab] = useState<ServerDialogTab>("general");

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
          <button
            aria-label="Close"
            className="panel-button panel-button-toolbar panel-button-icon dialog-close"
            onClick={onClose}
            type="button"
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
          </button>
        </header>

        <nav className="dialog-tabs" role="tablist">
          {(
            [
              { id: "general", label: "General" },
              { id: "containers", label: `Containers (${server.containers.length})` },
            ] as const
          ).map((tab) => (
            <button
              aria-selected={activeTab === tab.id}
              className={`dialog-tab ${activeTab === tab.id ? "is-active" : ""}`}
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              role="tab"
              type="button"
            >
              {tab.label}
            </button>
          ))}
        </nav>

        <div className="dialog-body">
          {activeTab === "general" ? (
            <GeneralTab server={server} />
          ) : (
            <ContainersTab server={server} />
          )}
        </div>
      </div>
    </div>
  );
}

function GeneralTab({ server }: { server: ManagedServer }) {
  return (
    <div className="dialog-tab-content">
      <div className="dialog-field-grid">
        <FieldRow label="ID" value={server.id} mono />
        <FieldRow label="Label" value={server.label} />
        <FieldRow label="Host" value={server.host} mono />
        <FieldRow label="Description" value={server.description} />
        <FieldRow label="State" value={server.state} badge />
        {server.message ? (
          <FieldRow label="Message" value={server.message} />
        ) : null}
        <FieldRow
          label="Containers"
          value={`${server.containers.length} container(s)`}
        />
      </div>
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
                <span
                  className={`dialog-badge dialog-badge-${container.status.toLowerCase()}`}
                >
                  {container.status}
                </span>
              </td>
              <td className="dialog-table-mono">{container.ipv4 || "—"}</td>
              <td>
                <span
                  className={`dialog-badge dialog-badge-${container.sshReachable ? "ready" : "unreachable"}`}
                >
                  {container.sshState}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function FieldRow({
  badge,
  label,
  mono,
  value,
}: {
  badge?: boolean;
  label: string;
  mono?: boolean;
  value: string;
}) {
  return (
    <div className="dialog-field">
      <span className="dialog-field-label">{label}</span>
      {badge ? (
        <span className={`dialog-badge dialog-badge-${value.toLowerCase()}`}>
          {value}
        </span>
      ) : (
        <span className={`dialog-field-value ${mono ? "dialog-field-mono" : ""}`}>
          {value}
        </span>
      )}
    </div>
  );
}
