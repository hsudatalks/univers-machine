import type { AppBootstrap } from "../types";
import { isMachineHostTarget, visibleContainers } from "../lib/container-visibility";
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

interface SidebarNavProps {
  activeMachineId?: string;
  activeTargetId?: string;
  availableTargetIds: string[];
  bootstrap: AppBootstrap;
  expandedMachineIds: string[];
  isDashboardActive: boolean;
  isOverviewActive: boolean;
  isOverviewLayout?: boolean;
  onSelectContainer: (targetId: string) => void;
  onSelectDashboard: () => void;
  onSelectOverview: () => void;
  onSelectMachine: (machineId: string) => void;
  onToggleMachine: (machineId: string) => void;
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
  activeMachineId,
  activeTargetId,
  availableTargetIds,
  bootstrap,
  expandedMachineIds,
  isDashboardActive,
  isOverviewActive,
  isOverviewLayout = false,
  onSelectContainer,
  onSelectDashboard,
  onSelectOverview,
  onSelectMachine,
  onToggleMachine,
}: SidebarNavProps) {
  const availableTargetSet = new Set(availableTargetIds);
  const managedTargetIds = new Set(
    bootstrap.machines.flatMap((machine) =>
      machine.containers.map((container) => container.targetId),
    ),
  );
  const standaloneTargets = bootstrap.targets.filter(
    (target) => !managedTargetIds.has(target.id) && !isMachineHostTarget(target),
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
          <SidebarGroupLabel>Machines</SidebarGroupLabel>

          <SidebarMenu className="sidebar-tree">
            {bootstrap.machines.map((machine) => {
              const isExpanded = expandedMachineIds.includes(machine.id);
              const isMachineActive = activeMachineId === machine.id;
              const branchHasActiveTarget =
                activeTargetId === machine.hostTargetId ||
                machine.containers.some(
                  (container) => container.targetId === activeTargetId,
                );
              const managedContainers = visibleContainers(machine.containers);

              return (
                <SidebarMenuItem className="sidebar-branch" key={machine.id}>
                  <div className="sidebar-branch-header">
                    <button
                      className="sidebar-branch-toggle"
                      onClick={() => onToggleMachine(machine.id)}
                      type="button"
                    >
                      <ChevronRight
                        className={isExpanded ? "rotate-90 transition-transform" : "transition-transform"}
                        size={14}
                      />
                    </button>

                    <SidebarMenuButton
                      className={branchHasActiveTarget ? "is-branch-active" : ""}
                      isActive={isMachineActive}
                      onClick={() => onSelectMachine(machine.id)}
                      type="button"
                    >
                      <span className="sidebar-node-copy">
                        <Server size={14} />
                        <span className="sidebar-node-label">{machine.label}</span>
                      </span>

                      <StatusBadge
                        state={inventoryStateTone(machine.state)}
                        title={titleCase(machine.state)}
                      />
                    </SidebarMenuButton>
                  </div>

                  {isExpanded ? (
                    <SidebarMenuSub className="sidebar-children">
                      {managedContainers.map((container) => {
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
