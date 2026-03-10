import type { AppBootstrap } from "../types";
import { Badge } from "./ui/badge";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
} from "./ui/sidebar";
import { ChevronRight, LayoutDashboard, LayoutGrid, Server, SquareTerminal } from "lucide-react";
import { serverHostTargetId } from "../lib/server-targets";

interface SidebarNavProps {
  activeServerId?: string;
  activeTargetId?: string;
  availableTargetIds: string[];
  bootstrap: AppBootstrap;
  expandedServerIds: string[];
  isDashboardActive: boolean;
  isOverviewActive: boolean;
  isOverviewLayout?: boolean;
  onSelectContainer: (targetId: string) => void;
  onSelectDashboard: () => void;
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
      return "success";
    case "degraded":
    case "empty":
      return "warning";
    case "error":
      return "destructive";
    default:
      return "neutral";
  }
}

function StatusBadge({ state, title }: { state: string; title: string }) {
  return (
    <Badge aria-label={title} title={title} variant={state as "neutral" | "success" | "warning" | "destructive"}>
      {title}
    </Badge>
  );
}

export function SidebarNav({
  activeServerId,
  activeTargetId,
  availableTargetIds,
  bootstrap,
  expandedServerIds,
  isDashboardActive,
  isOverviewActive,
  isOverviewLayout = false,
  onSelectContainer,
  onSelectDashboard,
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
    <Sidebar className={`sidebar ${isOverviewLayout ? "sidebar-overview" : ""}`}>
      <SidebarContent className="sidebar-nav" aria-label="Workspace navigation">
        <SidebarGroup>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                className="sidebar-node-root"
                isActive={isDashboardActive}
                onClick={onSelectDashboard}
                type="button"
              >
                <span className="sidebar-node-copy">
                  <LayoutDashboard size={14} />
                  <span className="sidebar-node-label">Dashboard</span>
                </span>
              </SidebarMenuButton>
            </SidebarMenuItem>
            <SidebarMenuItem>
              <SidebarMenuButton
                className="sidebar-node-root"
                isActive={isOverviewActive}
                onClick={onSelectOverview}
                type="button"
              >
                <span className="sidebar-node-copy">
                  <LayoutGrid size={14} />
                  <span className="sidebar-node-label">Overview</span>
                </span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroup>

        <SidebarGroup className="sidebar-section">
          <SidebarGroupLabel>Servers</SidebarGroupLabel>

          <SidebarMenu className="sidebar-tree">
            {bootstrap.servers.map((server) => {
              const isExpanded = expandedServerIds.includes(server.id);
              const isServerActive = activeServerId === server.id;
              const hostTargetId = serverHostTargetId(server.id);
              const isHostActive = activeTargetId === hostTargetId;
              const branchHasActiveTarget = server.containers.some(
                (container) => container.targetId === activeTargetId,
              ) || isHostActive;

              return (
                <SidebarMenuItem className="sidebar-branch" key={server.id}>
                  <div className="sidebar-branch-header">
                    <button
                      className="sidebar-branch-toggle"
                      onClick={() => onToggleServer(server.id)}
                      type="button"
                    >
                      <ChevronRight
                        className={isExpanded ? "rotate-90 transition-transform" : "transition-transform"}
                        size={14}
                      />
                    </button>

                    <SidebarMenuButton
                      className={branchHasActiveTarget ? "is-branch-active" : ""}
                      isActive={isServerActive}
                      onClick={() => onSelectServer(server.id)}
                      type="button"
                    >
                      <span className="sidebar-node-copy">
                        <Server size={14} />
                        <span className="sidebar-node-label">{server.label}</span>
                      </span>

                      <StatusBadge
                        state={inventoryStateTone(server.state)}
                        title={titleCase(server.state)}
                      />
                    </SidebarMenuButton>
                  </div>

                  {isExpanded ? (
                    <SidebarMenuSub className="sidebar-children">
                      <SidebarMenuButton
                        className="sidebar-node-leaf"
                        disabled={!availableTargetSet.has(hostTargetId)}
                        isActive={isHostActive}
                        onClick={() => onSelectContainer(hostTargetId)}
                        type="button"
                      >
                        <span className="sidebar-node-copy">
                          <Server size={14} />
                          <span className="sidebar-node-label">Host</span>
                        </span>

                        <StatusBadge
                          state={inventoryStateTone(server.state)}
                          title={titleCase(server.state)}
                        />
                      </SidebarMenuButton>

                      {server.containers.map((container) => {
                        const isActive = activeTargetId === container.targetId;
                        const isAvailable = availableTargetSet.has(container.targetId);

                        return (
                          <SidebarMenuButton
                            className="sidebar-node-leaf"
                            disabled={!isAvailable}
                            isActive={isActive}
                            key={container.targetId}
                            onClick={() => onSelectContainer(container.targetId)}
                            type="button"
                          >
                            <span className="sidebar-node-copy">
                              <SquareTerminal size={14} />
                              <span className="sidebar-node-label">{container.label}</span>
                            </span>

                            <StatusBadge
                              state={container.sshReachable ? "success" : "destructive"}
                              title={container.sshState}
                            />
                          </SidebarMenuButton>
                        );
                      })}
                    </SidebarMenuSub>
                  ) : null}
                </SidebarMenuItem>
              );
            })}
          </SidebarMenu>
        </SidebarGroup>

        {standaloneTargets.length > 0 ? (
          <SidebarGroup className="sidebar-section">
            <SidebarGroupLabel>Standalone</SidebarGroupLabel>

            <SidebarMenu className="sidebar-tree">
              {standaloneTargets.map((target) => (
                <SidebarMenuButton
                  className="sidebar-node-leaf"
                  isActive={activeTargetId === target.id}
                  key={target.id}
                  onClick={() => onSelectContainer(target.id)}
                  type="button"
                >
                  <span className="sidebar-node-copy">
                    <SquareTerminal size={14} />
                    <span className="sidebar-node-label">{target.label}</span>
                  </span>
                </SidebarMenuButton>
              ))}
            </SidebarMenu>
          </SidebarGroup>
        ) : null}
      </SidebarContent>
    </Sidebar>
  );
}
