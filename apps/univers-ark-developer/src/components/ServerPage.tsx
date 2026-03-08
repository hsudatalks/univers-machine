import { TerminalCard } from "./TerminalCard";
import type { DeveloperTarget, ManagedContainer, ManagedServer } from "../types";

interface ServerPageProps {
  onOpenWorkspace: (targetId: string) => void;
  pageVisible: boolean;
  resolveTarget: (targetId: string) => DeveloperTarget | undefined;
  server: ManagedServer;
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

export function ServerPage({
  onOpenWorkspace,
  pageVisible,
  resolveTarget,
  server,
}: ServerPageProps) {
  const reachableContainers = server.containers.filter(
    (container) => container.sshReachable,
  ).length;

  return (
    <>
      <header className="content-header">
        <div className="content-header-copy">
          <span className="panel-title">Server</span>
          <h1 className="content-title">{server.label}</h1>
          <p className="panel-description">{server.description}</p>
        </div>

        <div className="content-meta-row">
          <span className="content-chip">{server.host}</span>
          <span className="content-chip">{server.containers.length} container(s)</span>
          <span className="content-chip">{reachableContainers} SSH ready</span>
        </div>
      </header>

      <section className="page-section">
        <div className="terminal-grid">
          {server.containers.map((container) => {
            const target = resolveTarget(container.targetId);

            return target ? (
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
          })}
        </div>
      </section>
    </>
  );
}
