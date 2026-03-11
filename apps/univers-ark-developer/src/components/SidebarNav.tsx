import type { AppBootstrap } from "../types";
import { isMachineHostTarget, visibleContainers } from "../lib/container-visibility";
import { ConnectionStatusLight } from "./ConnectionStatusLight";
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
                  <span className="sidebar-node-label">Orchestration</span>
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
              const managedContainers = visibleContainers(machine.containers);
              const hasManagedContainers = managedContainers.length > 0;
              const branchHasActiveTarget =
                activeTargetId === machine.hostTargetId ||
                machine.containers.some(
                  (container) => container.targetId === activeTargetId,
                );

              return (
                <SidebarMenuItem className="sidebar-branch" key={machine.id}>
                  <div
                    className={`sidebar-branch-header ${hasManagedContainers ? "" : "sidebar-branch-header-flat"}`.trim()}
                  >
                    {hasManagedContainers ? (
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
                    ) : null}

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

                      <ConnectionStatusLight
                        title={titleCase(machine.state)}
                        state={machine.state}
                      />
                    </SidebarMenuButton>
                  </div>

                  {isExpanded && hasManagedContainers ? (
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

                            <ConnectionStatusLight
                              state={container.sshState}
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
