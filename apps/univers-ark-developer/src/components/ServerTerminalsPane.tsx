import { connectionStatusClass } from "../lib/connectivity-state";
import { visibleContainers } from "../lib/container-visibility";
import { TerminalCard } from "./TerminalCard";
import type { DeveloperTarget, ManagedContainer, ManagedMachine } from "../types";

interface ServerTerminalsPaneProps {
  activeFocusedTargetId?: string;
  onFocusTarget: (targetId: string) => void;
  onOpenWorkspace: (targetId: string) => void;
  pageVisible: boolean;
  registerTerminalElement?: (targetId: string, element: HTMLElement | null) => void;
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
          <span className={`terminal-status ${connectionStatusClass(container.sshState)}`}>
            {container.sshState}
          </span>
        </div>
      </header>

      <div className="terminal-placeholder-body">
        <p className="terminal-placeholder-copy">{container.sshMessage}</p>
      </div>
    </article>
  );
}

export function ServerTerminalsPane({
  activeFocusedTargetId,
  onFocusTarget,
  onOpenWorkspace,
  pageVisible,
  registerTerminalElement,
  resolveTarget,
  server,
}: ServerTerminalsPaneProps) {
  const managedContainers = visibleContainers(server.containers);

  return (
    <article className="panel tool-panel server-terminals-panel">
      <div className="server-terminal-grid">
        {managedContainers.length ? (
          managedContainers.map((container) => {
            const target = resolveTarget(container.targetId);

            return target && container.sshReachable ? (
              <TerminalCard
                isGridFocused={container.targetId === activeFocusedTargetId}
                key={container.targetId}
                onFocusRequest={() => {
                  onFocusTarget(container.targetId);
                }}
                onOpenWorkspace={() => {
                  onOpenWorkspace(target.id);
                }}
                pageVisible={pageVisible}
                registerElement={(element) => {
                  registerTerminalElement?.(container.targetId, element);
                }}
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
              This provider does not currently expose any managed containers.
            </p>
          </section>
        )}
      </div>
    </article>
  );
}
