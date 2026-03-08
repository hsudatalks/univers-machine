import {
  useEffect,
  useEffectEvent,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { restartTunnel } from "../lib/tauri";
import { warmTargetTunnels } from "../lib/tunnel-manager";
import {
  browserSurfaceIdFromPanel,
  isBrowserToolPanel,
  type ActiveView,
  type ContainerToolPanel,
} from "../lib/view-types";
import type { DeveloperTarget, TunnelStatus } from "../types";

const IS_MAC = navigator.platform.toUpperCase().includes("MAC");

function isPlatformModifier(event: KeyboardEvent): boolean {
  return IS_MAC ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

interface ResizeSession {
  targetId: string;
  startTerminalWidth: number;
  startPointerX: number;
  workspaceWidth: number;
}

interface UseContainerWorkspaceOptions {
  activeView: ActiveView;
  clampTerminalPanelWidth: (value: number, workspaceWidth: number) => number;
  defaultTerminalPanelWidthPx: () => number;
  onSetActiveView: (view: ActiveView) => void;
  onSelectContainer: (targetId: string) => void;
  onSetOverviewFocus: (targetId: string) => void;
  onTunnelStatus: (status: TunnelStatus) => void;
  orderedTargetIds: string[];
  targetById: Map<string, DeveloperTarget>;
}

export function useContainerWorkspace({
  activeView,
  clampTerminalPanelWidth,
  defaultTerminalPanelWidthPx,
  onSetActiveView,
  onSelectContainer,
  onSetOverviewFocus,
  onTunnelStatus,
  orderedTargetIds,
  targetById,
}: UseContainerWorkspaceOptions) {
  const [containerTools, setContainerTools] = useState<
    Record<string, ContainerToolPanel | undefined>
  >({});
  const [containerTerminalWidths, setContainerTerminalWidths] = useState<
    Record<string, number>
  >({});
  const [browserFrameVersions, setBrowserFrameVersions] = useState<
    Record<string, number>
  >({});
  const [activeResize, setActiveResize] = useState<ResizeSession | null>(null);

  useEffect(() => {
    if (!activeResize) {
      return;
    }

    const handlePointerMove = (event: PointerEvent) => {
      const deltaX = event.clientX - activeResize.startPointerX;
      const nextWidth = clampTerminalPanelWidth(
        activeResize.startTerminalWidth + deltaX,
        activeResize.workspaceWidth,
      );

      setContainerTerminalWidths((current) => ({
        ...current,
        [activeResize.targetId]: nextWidth,
      }));
    };

    const handlePointerUp = () => {
      setActiveResize(null);
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
  }, [activeResize, clampTerminalPanelWidth]);

  const selectContainerTool = (
    target: DeveloperTarget,
    panel: ContainerToolPanel,
  ) => {
    setContainerTools((current) => ({
      ...current,
      [target.id]: panel,
    }));

    if (!(target.id in containerTerminalWidths)) {
      setContainerTerminalWidths((current) => ({
        ...current,
        [target.id]: defaultTerminalPanelWidthPx(),
      }));
    }

    if (isBrowserToolPanel(panel)) {
      const surfaceId = browserSurfaceIdFromPanel(panel);
      const surface = target.surfaces.find((entry) => entry.id === surfaceId);

      if (surface) {
        warmTargetTunnels(target, [surface.id], onTunnelStatus);
      }
    }
  };

  const selectContainerToolFromShortcut = useEffectEvent(
    (target: DeveloperTarget, panel: ContainerToolPanel) => {
      selectContainerTool(target, panel);
    },
  );
  const selectContainerFromShortcut = useEffectEvent((targetId: string) => {
    onSelectContainer(targetId);
  });

  useEffect(() => {
    if (activeView.kind !== "container") {
      return;
    }

    const { targetId } = activeView;
    const target = targetById.get(targetId);

    if (!target) {
      return;
    }

    const developmentSurface = target.surfaces.find(
      (surface) => surface.id === "development",
    );
    const previewSurface = target.surfaces.find((surface) => surface.id === "preview");
    const developmentPanel = developmentSurface
      ? (`browser:${developmentSurface.id}` as const)
      : null;
    const previewPanel = previewSurface
      ? (`browser:${previewSurface.id}` as const)
      : null;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isPlatformModifier(event) || event.altKey || event.shiftKey) {
        return;
      }

      if (event.key === "Backspace" || event.key === "Delete") {
        event.preventDefault();
        event.stopPropagation();
        onSetOverviewFocus(targetId);
        onSetActiveView({ kind: "overview" });
        return;
      }

      if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        const currentIndex = orderedTargetIds.indexOf(targetId);

        if (currentIndex === -1) {
          return;
        }

        const nextIndex =
          event.key === "ArrowLeft" ? currentIndex - 1 : currentIndex + 1;
        const nextTargetId = orderedTargetIds[nextIndex];

        if (!nextTargetId) {
          return;
        }

        event.preventDefault();
        event.stopPropagation();
        selectContainerFromShortcut(nextTargetId);
        return;
      }

      let nextPanel: ContainerToolPanel | null = null;

      switch (event.key) {
        case "1":
          nextPanel = "files";
          break;
        case "2":
          nextPanel = developmentPanel;
          break;
        case "3":
          nextPanel = previewPanel;
          break;
        default:
          return;
      }

      if (!nextPanel) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      selectContainerToolFromShortcut(target, nextPanel);
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [
    activeView,
    onSetActiveView,
    onSelectContainer,
    onSetOverviewFocus,
    orderedTargetIds,
    targetById,
  ]);

  function prepareContainerView(target: DeveloperTarget) {
    setContainerTools((current) => ({
      ...current,
      [target.id]: current[target.id] ?? "files",
    }));
    setContainerTerminalWidths((current) => ({
      ...current,
      [target.id]: current[target.id] ?? defaultTerminalPanelWidthPx(),
    }));
    warmTargetTunnels(target, undefined, onTunnelStatus);
  }

  function startContainerResize(
    event: ReactPointerEvent<HTMLDivElement>,
    targetId: string,
  ) {
    const workspace = event.currentTarget.parentElement;

    if (!workspace) {
      return;
    }

    const workspaceWidth = workspace.getBoundingClientRect().width;
    const startTerminalWidth =
      containerTerminalWidths[targetId] ?? defaultTerminalPanelWidthPx();

    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    setActiveResize({
      targetId,
      startTerminalWidth,
      startPointerX: event.clientX,
      workspaceWidth,
    });
  }

  function reloadBrowserFrame(targetId: string, surfaceId: string) {
    const key = `${targetId}::${surfaceId}`;

    setBrowserFrameVersions((current) => ({
      ...current,
      [key]: (current[key] ?? 0) + 1,
    }));
  }

  function restartBrowserTunnel(targetId: string, surfaceId: string) {
    void restartTunnel(targetId, surfaceId)
      .then((status) => {
        onTunnelStatus(status);
      })
      .catch((restartError) => {
        onTunnelStatus({
          targetId,
          surfaceId,
          state: "error",
          message:
            restartError instanceof Error
              ? restartError.message
              : "Failed to restart browser tunnel.",
        });
      });
  }

  return {
    browserFrameVersions,
    containerTerminalWidths,
    containerTools,
    prepareContainerView,
    reloadBrowserFrame,
    restartBrowserTunnel,
    selectContainerTool,
    startContainerResize,
  };
}
