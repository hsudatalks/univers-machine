import type { AppBootstrap } from "../types";

interface SidebarNavProps {
  activeServerId?: string;
  activeTargetId?: string;
  availableTargetIds: string[];
  bootstrap: AppBootstrap;
  expandedServerIds: string[];
  isOverviewActive: boolean;
  isOverviewLayout?: boolean;
  onSelectContainer: (targetId: string) => void;
  onSelectOverview: () => void;
  onSelectServer: (serverId: string) => void;
  onToggleServer: (serverId: string) => void;
}

function titleCase(value: string): string {
  if (!value) {
    return "";
  }

  return value.slice(0, 1).toUpperCase() + value.slice(1);
}

function inventoryStateTone(state: string): string {
  switch (state) {
    case "ready":
      return "running";
    case "degraded":
    case "empty":
      return "starting";
    case "error":
      return "error";
    default:
      return "direct";
  }
}

function StatusDot({
  state,
  title,
}: {
  state: string;
  title: string;
}) {
  return (
    <span
      aria-label={title}
      className={`terminal-status terminal-status-dot status-${state}`}
      title={title}
    />
  );
}

export function SidebarNav({
  activeServerId,
  activeTargetId,
  availableTargetIds,
  bootstrap,
  expandedServerIds,
  isOverviewActive,
  isOverviewLayout = false,
  onSelectContainer,
  onSelectOverview,
  onSelectServer,
  onToggleServer,
}: SidebarNavProps) {
  const availableTargetSet = new Set(availableTargetIds);
  const managedTargetIds = new Set(
    bootstrap.servers.flatMap((server) =>
      server.containers.map((container) => container.targetId),
    ),
  );
  const standaloneTargets = bootstrap.targets.filter(
    (target) => !managedTargetIds.has(target.id),
  );

  return (
    <aside className={`sidebar ${isOverviewLayout ? "sidebar-overview" : ""}`}>
      <nav className="sidebar-nav" aria-label="Workspace navigation">
        <button
          className={`sidebar-node sidebar-node-root ${isOverviewActive ? "is-active" : ""}`}
          onClick={onSelectOverview}
          type="button"
        >
          <span className="sidebar-node-copy">
            <span className="sidebar-node-label">Overview</span>
          </span>
        </button>

        <section className="sidebar-section">
          <span className="sidebar-section-label">Servers</span>

          <div className="sidebar-tree">
            {bootstrap.servers.map((server) => {
              const isExpanded = expandedServerIds.includes(server.id);
              const isServerActive = activeServerId === server.id;
              const branchHasActiveTarget = server.containers.some(
                (container) => container.targetId === activeTargetId,
              );

              return (
                <div className="sidebar-branch" key={server.id}>
                  <div className="sidebar-branch-header">
                    <button
                      className="sidebar-branch-toggle"
                      onClick={() => onToggleServer(server.id)}
                      type="button"
                    >
                      {isExpanded ? "▾" : "▸"}
                    </button>

                    <button
                      className={`sidebar-node sidebar-node-server ${isServerActive ? "is-active" : ""} ${branchHasActiveTarget ? "is-branch-active" : ""}`}
                      onClick={() => onSelectServer(server.id)}
                      type="button"
                    >
                      <span className="sidebar-node-copy">
                        <span className="sidebar-node-label">{server.label}</span>
                      </span>

                      <StatusDot
                        state={inventoryStateTone(server.state)}
                        title={titleCase(server.state)}
                      />
                    </button>
                  </div>

                  {isExpanded ? (
                    <div className="sidebar-children">
                      {server.containers.map((container) => {
                        const isActive = activeTargetId === container.targetId;
                        const isAvailable = availableTargetSet.has(container.targetId);

                        return (
                          <button
                            className={`sidebar-node sidebar-node-leaf ${isActive ? "is-active" : ""}`}
                            disabled={!isAvailable}
                            key={container.targetId}
                            onClick={() => onSelectContainer(container.targetId)}
                            type="button"
                          >
                            <span className="sidebar-node-copy">
                              <span className="sidebar-node-label">{container.label}</span>
                            </span>

                            <StatusDot
                              state={container.sshReachable ? "running" : "error"}
                              title={container.sshState}
                            />
                          </button>
                        );
                      })}
                    </div>
                  ) : null}
                </div>
              );
            })}
          </div>
        </section>

        {standaloneTargets.length > 0 ? (
          <section className="sidebar-section">
            <span className="sidebar-section-label">Standalone</span>

            <div className="sidebar-tree">
              {standaloneTargets.map((target) => (
                <button
                  className={`sidebar-node sidebar-node-leaf ${activeTargetId === target.id ? "is-active" : ""}`}
                  key={target.id}
                  onClick={() => onSelectContainer(target.id)}
                  type="button"
                >
                  <span className="sidebar-node-copy">
                    <span className="sidebar-node-label">{target.label}</span>
                  </span>
                </button>
              ))}
            </div>
          </section>
        ) : null}
      </nav>
    </aside>
  );
}
