import type { AppBootstrap } from "../types";

interface SidebarNavProps {
  activeServerId?: string;
  activeTargetId?: string;
  availableTargetIds: string[];
  bootstrap: AppBootstrap;
  expandedServerIds: string[];
  isOverviewActive: boolean;
  isRefreshing: boolean;
  onRefresh: () => void;
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

export function SidebarNav({
  activeServerId,
  activeTargetId,
  availableTargetIds,
  bootstrap,
  expandedServerIds,
  isOverviewActive,
  isRefreshing,
  onRefresh,
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
    <aside className="sidebar">
      <header className="sidebar-header">
        <div className="sidebar-brand">
          <span className="panel-title">Univers Ark</span>
          <h1 className="sidebar-title">Developer</h1>
          <p className="panel-description">
            Overview and per-server navigation for remote development containers.
          </p>
        </div>

        <button
          className="panel-button"
          disabled={isRefreshing}
          onClick={onRefresh}
          type="button"
        >
          {isRefreshing ? "Refreshing" : "Refresh"}
        </button>
      </header>

      <nav className="sidebar-nav" aria-label="Workspace navigation">
        <button
          className={`sidebar-node sidebar-node-root ${isOverviewActive ? "is-active" : ""}`}
          onClick={onSelectOverview}
          type="button"
        >
          <span className="sidebar-node-copy">
            <span className="sidebar-node-label">Overview</span>
            <span className="sidebar-node-meta">
              All container terminals in one place
            </span>
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
                        <span className="sidebar-node-meta">
                          {server.containers.length} container(s)
                        </span>
                      </span>

                      <span
                        className={`terminal-status status-${inventoryStateTone(server.state)}`}
                      >
                        {titleCase(server.state)}
                      </span>
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
                              <span className="sidebar-node-label">
                                {container.label}
                              </span>
                              <span className="sidebar-node-meta">
                                {container.ipv4 || container.sshDestination}
                              </span>
                            </span>

                            <span
                              className={`terminal-status status-${container.sshReachable ? "running" : "error"}`}
                            >
                              {container.sshState}
                            </span>
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
                    <span className="sidebar-node-meta">{target.host}</span>
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
