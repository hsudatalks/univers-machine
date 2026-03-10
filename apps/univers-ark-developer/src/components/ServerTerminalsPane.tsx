import { TerminalCard } from "./TerminalCard";
import type { DeveloperTarget, ManagedContainer, ManagedMachine } from "../types";

interface ServerTerminalsPaneProps {
  onOpenWorkspace: (targetId: string) => void;
  pageVisible: boolean;
  resolveTarget: (targetId: string) => DeveloperTarget | undefined;
  server: ManagedMachine;
}

function UnavailableTerminalCard({ container }: { container: ManagedContainer }) {
  return (
    <article className="panel terminal-card terminal-card-unavailable">
      <header className="panel-header terminal-placeholder-header">
        <div className="terminal-copy">
          <span className="panel-title">{container.label}</span>
        </div>

        <div className="terminal-meta">
          <span className="terminal-status status-error">{container.sshState}</span>
        </div>
      </header>

      <div className="terminal-placeholder-body">
        <p className="terminal-placeholder-copy">{container.sshMessage}</p>
      </div>
    </article>
  );
}

export function ServerTerminalsPane({
  onOpenWorkspace,
  pageVisible,
  resolveTarget,
  server,
}: ServerTerminalsPaneProps) {
  return (
    <article className="panel tool-panel server-terminals-panel">
      <div className="server-terminal-grid">
        {server.containers.length ? (
          server.containers.map((container) => {
            const target = resolveTarget(container.targetId);

            return target && container.sshReachable ? (
              <TerminalCard
                key={container.targetId}
                onOpenWorkspace={() => {
                  onOpenWorkspace(target.id);
                }}
                pageVisible={pageVisible}
                target={target}
                title={container.label}
              />
            ) : (
              <UnavailableTerminalCard container={container} key={container.targetId} />
            );
          })
        ) : (
          <section className="server-empty-state">
            <span className="state-label">No containers</span>
            <p className="state-copy">
              This machine does not currently expose any managed containers.
            </p>
          </section>
        )}
      </div>
    </article>
  );
}
