import type {
  CSSProperties,
  PointerEvent as ReactPointerEvent,
} from "react";
import { ContainerPage } from "../../components/ContainerPage";
import type { ContainerToolPanel } from "../../lib/view-types";
import {
  browserSurfaceIdFromPanel,
  isBrowserToolPanel,
} from "../../lib/view-types";
import {
  browserSurfaceById,
  primaryBrowserSurface,
  resolveDefaultToolPanel,
  webServices,
} from "../../lib/target-services";
import type {
  BrowserFrameInstance,
} from "../../components/BrowserPane";
import type {
  DeveloperSurface,
  DeveloperTarget,
  ServiceStatus,
  TunnelStatus,
} from "../../types";

interface WorkbenchPagesProps {
  activeTargetId: string | null;
  browserFrameVersions: Record<string, number>;
  containerTerminalCollapsed: Record<string, boolean | undefined>;
  containerTerminalWidths: Record<string, number>;
  containerTools: Record<string, ContainerToolPanel | undefined>;
  dashboardRefreshSeconds: number;
  defaultTerminalPanelWidthPx: () => number;
  onExecuteCommandService: (
    targetId: string,
    serviceId: string,
    action: "restart",
  ) => Promise<void>;
  onResetBrowser: (targetId: string, surfaceId: string) => void;
  onRestartContainer: (machineId: string, containerId: string) => Promise<void>;
  onSelectTool: (target: DeveloperTarget, panel: ContainerToolPanel) => void;
  onStartResize: (
    event: ReactPointerEvent<HTMLDivElement>,
    targetId: string,
  ) => void;
  onToggleTerminalCollapsed: (targetId: string) => void;
  serviceStatuses: Record<string, ServiceStatus>;
  targetById: Map<string, DeveloperTarget>;
  tunnelStatuses: Record<string, TunnelStatus>;
  visitedTargetIds: string[];
}

function surfaceKey(targetId: string, surfaceId: string): string {
  return `${targetId}::${surfaceId}`;
}

function fallbackTunnelStatus(
  target: DeveloperTarget,
  surface: DeveloperSurface,
): TunnelStatus {
  if (target.transport === "local" && !surface.tunnelCommand.trim()) {
    return {
      targetId: target.id,
      serviceId: surface.id,
      surfaceId: surface.id,
      localUrl: surface.localUrl,
      state: "direct",
      message: `${surface.label} is available directly without a managed tunnel.`,
    };
  }

  return {
    targetId: target.id,
    serviceId: surface.id,
    surfaceId: surface.id,
    localUrl: surface.localUrl,
    state: "starting",
    message: `${surface.label} is warming in the background.`,
  };
}

function containerViewRefreshKey(target: DeveloperTarget): string {
  return JSON.stringify({
    label: target.label,
    description: target.description,
    notes: target.notes,
    services: target.services,
    surfaces: target.surfaces,
    terminalCommand: target.terminalCommand,
    terminalStartupCommand: target.terminalStartupCommand,
    workspace: target.workspace,
  });
}

export function WorkbenchPages({
  activeTargetId,
  browserFrameVersions,
  containerTerminalCollapsed,
  containerTerminalWidths,
  containerTools,
  dashboardRefreshSeconds,
  defaultTerminalPanelWidthPx,
  onExecuteCommandService,
  onResetBrowser,
  onRestartContainer,
  onSelectTool,
  onStartResize,
  onToggleTerminalCollapsed,
  serviceStatuses,
  targetById,
  tunnelStatuses,
  visitedTargetIds,
}: WorkbenchPagesProps) {
  return (
    <>
      {visitedTargetIds.map((targetId) => {
        const target = targetById.get(targetId);
        const isVisible = activeTargetId === targetId;
        const activeTool = target
          ? (containerTools[target.id] ?? resolveDefaultToolPanel(target))
          : "dashboard";
        const primarySurface = target
          ? primaryBrowserSurface(target)
          : undefined;
        const activeBrowserSurfaceId = browserSurfaceIdFromPanel(activeTool);
        const browserSurface =
          activeBrowserSurfaceId && target
            ? browserSurfaceById(target, activeBrowserSurfaceId)
            : undefined;
        const browserPanel = browserSurface
          ? (`browser:${browserSurface.id}` as const)
          : primarySurface
            ? (`browser:${primarySurface.id}` as const)
            : isBrowserToolPanel(activeTool)
              ? activeTool
              : null;
        const browserStatus =
          browserSurface && target
            ? tunnelStatuses[surfaceKey(target.id, browserSurface.id)] ??
              fallbackTunnelStatus(target, browserSurface)
            : undefined;
        const browserFrames: BrowserFrameInstance[] = target
          ? webServices(target).map((service) => {
              const surface = service.web;
              const panel = `browser:${surface.id}` as const;
              const status =
                tunnelStatuses[surfaceKey(target.id, surface.id)] ??
                fallbackTunnelStatus(target, surface);

              return {
                cacheKey: surfaceKey(target.id, surface.id),
                frameVersion:
                  browserFrameVersions[surfaceKey(target.id, surface.id)] ?? 0,
                isActive: isVisible && activeTool === panel,
                status,
                surface,
                target,
              };
            })
          : [];
        const browserFrame: BrowserFrameInstance | undefined =
          browserSurface && browserStatus && target
            ? browserFrames.find(
                (frame) => frame.surface.id === browserSurface.id,
              )
            : undefined;
        const primaryBrowserStatus =
          primarySurface && target
            ? tunnelStatuses[surfaceKey(target.id, primarySurface.id)] ??
              fallbackTunnelStatus(target, primarySurface)
            : undefined;

        return (
          <section
            className={`content-page content-page-container ${
              isVisible ? "" : "is-hidden"
            }`}
            key={targetId}
          >
            {target ? (
              <ContainerPage
                activeTool={activeTool}
                browserFrame={browserFrame}
                browserFrames={browserFrames}
                browserPanel={browserPanel}
                browserServices={webServices(target).map((service) => ({
                  id: service.id,
                  label: service.label,
                }))}
                browserSurface={browserSurface}
                dashboardRefreshSeconds={dashboardRefreshSeconds}
                isTerminalCollapsed={Boolean(containerTerminalCollapsed[target.id])}
                onExecuteCommandService={(serviceId, action) =>
                  onExecuteCommandService(target.id, serviceId, action)
                }
                onOpenBrowserService={(serviceId) => {
                  onSelectTool(target, `browser:${serviceId}`);
                }}
                onResetBrowser={() => {
                  if (browserSurface) {
                    onResetBrowser(target.id, browserSurface.id);
                  }
                }}
                onRestartContainer={
                  target.containerKind === "managed"
                    ? async () => {
                        await onRestartContainer(target.machineId, target.containerId);
                      }
                    : undefined
                }
                onSelectTool={(panel) => {
                  onSelectTool(target, panel);
                }}
                onStartResize={(event) => {
                  onStartResize(event, target.id);
                }}
                onToggleTerminalCollapsed={() => {
                  onToggleTerminalCollapsed(target.id);
                }}
                pageVisible={isVisible}
                primaryBrowserStatus={primaryBrowserStatus}
                primaryBrowserSurface={primarySurface}
                serviceStatuses={serviceStatuses}
                target={target}
                workspaceStyle={{
                  "--container-terminal-width": `${
                    containerTerminalWidths[target.id] ??
                    defaultTerminalPanelWidthPx()
                  }px`,
                } as CSSProperties}
                key={`${target.id}:${containerViewRefreshKey(target)}`}
              />
            ) : (
              <section className="state-panel">
                <span className="state-label">Unavailable</span>
                <p className="state-copy">
                  The selected navigation target is no longer available.
                </p>
              </section>
            )}
          </section>
        );
      })}
    </>
  );
}
