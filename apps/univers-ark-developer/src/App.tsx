import {
  useEffect,
  useEffectEvent,
  useMemo,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { isTauri } from "@tauri-apps/api/core";
import { BrowserPane, type BrowserFrameInstance } from "./components/BrowserPane";
import { FilesPane } from "./components/FilesPane";
import { SidebarNav } from "./components/SidebarNav";
import { TerminalCard } from "./components/TerminalCard";
import { TerminalPane } from "./components/TerminalPane";
import "./App.css";
import {
  listenTunnelStatus,
  listenSidebarToggleRequested,
  loadBootstrap,
  restartTunnel,
} from "./lib/tauri";
import { warmTargetTunnels } from "./lib/tunnel-manager";
import type {
  AppBootstrap,
  DeveloperSurface,
  DeveloperTarget,
  ManagedContainer,
  ManagedServer,
  TunnelStatus,
} from "./types";

type ActiveView =
  | { kind: "overview" }
  | { kind: "server"; serverId: string }
  | { kind: "container"; targetId: string };

type ContainerToolPanel = "files" | `browser:${string}`;

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

const OVERVIEW_ZOOM_STORAGE_KEY = "univers-ark-developer:overview-zoom";
const SIDEBAR_VISIBILITY_STORAGE_KEY =
  "univers-ark-developer:sidebar-hidden";
const OVERVIEW_ZOOM_MIN = 0.8;
const OVERVIEW_ZOOM_MAX = 1.3;
const OVERVIEW_ZOOM_STEP = 0.1;
const OVERVIEW_ZOOM_DEFAULT = 1;
const DEFAULT_TERMINAL_PANEL_WIDTH_REM = 25;
const MIN_TERMINAL_PANEL_WIDTH_REM = 25;
const MIN_TOOL_PANEL_WIDTH_REM = 22;

function resolvePreferredTarget(
  bootstrap: AppBootstrap,
  preferredTargetId?: string,
): DeveloperTarget | undefined {
  if (preferredTargetId) {
    const preferredTarget = bootstrap.targets.find(
      (target) => target.id === preferredTargetId,
    );

    if (preferredTarget) {
      return preferredTarget;
    }
  }

  return (
    bootstrap.targets.find(
      (target) => target.id === bootstrap.selectedTargetId,
    ) ?? bootstrap.targets[0]
  );
}

function uniqueStrings(values: string[]): string[] {
  const seen = new Set<string>();

  return values.filter((value) => {
    if (seen.has(value)) {
      return false;
    }

    seen.add(value);
    return true;
  });
}

function clampOverviewZoom(value: number): number {
  return Math.min(OVERVIEW_ZOOM_MAX, Math.max(OVERVIEW_ZOOM_MIN, value));
}

function roundOverviewZoom(value: number): number {
  return Math.round(value * 10) / 10;
}

function serverForTargetId(
  servers: ManagedServer[],
  targetId: string,
): ManagedServer | undefined {
  return servers.find((server) =>
    server.containers.some((container) => container.targetId === targetId),
  );
}

function surfaceKey(targetId: string, surfaceId: string): string {
  return `${targetId}::${surfaceId}`;
}

function fallbackTunnelStatus(
  targetId: string,
  surface: DeveloperSurface,
): TunnelStatus {
  if (!surface.tunnelCommand.trim()) {
    return {
      targetId,
      surfaceId: surface.id,
      state: "direct",
      message: `${surface.label} is available directly without a managed tunnel.`,
    };
  }

  return {
    targetId,
    surfaceId: surface.id,
    state: "starting",
    message: `${surface.label} is warming in the background.`,
  };
}

function rootFontSizePx(): number {
  if (typeof window === "undefined") {
    return 16;
  }

  const parsed = Number.parseFloat(
    window.getComputedStyle(document.documentElement).fontSize,
  );

  return Number.isFinite(parsed) ? parsed : 16;
}

function defaultTerminalPanelWidthPx(): number {
  return DEFAULT_TERMINAL_PANEL_WIDTH_REM * rootFontSizePx();
}

function minTerminalPanelWidthPx(): number {
  return MIN_TERMINAL_PANEL_WIDTH_REM * rootFontSizePx();
}

function minToolPanelWidthPx(): number {
  return MIN_TOOL_PANEL_WIDTH_REM * rootFontSizePx();
}

function clampTerminalPanelWidth(value: number, workspaceWidth: number): number {
  const minTerminalWidth = minTerminalPanelWidthPx();
  const maxTerminalWidth = Math.max(
    minTerminalWidth,
    workspaceWidth - minToolPanelWidthPx(),
  );

  return Math.min(maxTerminalWidth, Math.max(minTerminalWidth, value));
}

function isBrowserToolPanel(
  panel: ContainerToolPanel | null | undefined,
): panel is `browser:${string}` {
  return Boolean(panel?.startsWith("browser:"));
}

function browserSurfaceIdFromPanel(
  panel: ContainerToolPanel | null | undefined,
): string | null {
  if (!isBrowserToolPanel(panel)) {
    return null;
  }

  return panel.slice("browser:".length) || null;
}

type OverviewMoveDirection = "left" | "right" | "up" | "down";

function adjacentOverviewTargetId(
  direction: OverviewMoveDirection,
  currentTargetId: string,
  targetIds: string[],
  elements: Map<string, HTMLElement>,
): string {
  const cards = targetIds
    .map((targetId) => {
      const element = elements.get(targetId);

      if (!element) {
        return null;
      }

      const rect = element.getBoundingClientRect();

      return {
        centerX: rect.left + rect.width / 2,
        centerY: rect.top + rect.height / 2,
        id: targetId,
      };
    })
    .filter(
      (
        card,
      ): card is {
        centerX: number;
        centerY: number;
        id: string;
      } => Boolean(card),
    );

  if (cards.length === 0) {
    return currentTargetId;
  }

  const currentCard = cards.find((card) => card.id === currentTargetId) ?? cards[0];
  const candidates = cards.filter((card) => {
    switch (direction) {
      case "left":
        return card.centerX < currentCard.centerX - 4;
      case "right":
        return card.centerX > currentCard.centerX + 4;
      case "up":
        return card.centerY < currentCard.centerY - 4;
      case "down":
        return card.centerY > currentCard.centerY + 4;
    }
  });

  if (candidates.length === 0) {
    return currentCard.id;
  }

  const perpendicularWeight = 4;

  const bestCandidate = candidates.reduce((best, candidate) => {
    const axisDistance =
      direction === "left" || direction === "right"
        ? Math.abs(candidate.centerX - currentCard.centerX)
        : Math.abs(candidate.centerY - currentCard.centerY);
    const perpendicularDistance =
      direction === "left" || direction === "right"
        ? Math.abs(candidate.centerY - currentCard.centerY)
        : Math.abs(candidate.centerX - currentCard.centerX);
    const score = axisDistance + perpendicularDistance * perpendicularWeight;

    if (!best || score < best.score) {
      return {
        id: candidate.id,
        score,
      };
    }

    return best;
  }, null as { id: string; score: number } | null);

  return bestCandidate?.id ?? currentCard.id;
}

function App() {
  const [bootstrap, setBootstrap] = useState<AppBootstrap | null>(null);
  const [activeView, setActiveView] = useState<ActiveView>({ kind: "overview" });
  const [visitedContainerIds, setVisitedContainerIds] = useState<string[]>([]);
  const [visitedServerIds, setVisitedServerIds] = useState<string[]>([]);
  const [overviewFocusedTargetId, setOverviewFocusedTargetId] = useState<string>("");
  const [isSidebarHidden, setIsSidebarHidden] = useState(() => {
    if (typeof window === "undefined") {
      return false;
    }

    return (
      window.localStorage.getItem(SIDEBAR_VISIBILITY_STORAGE_KEY) === "true"
    );
  });
  const [expandedServerIds, setExpandedServerIds] = useState<string[]>([]);
  const [containerTools, setContainerTools] = useState<
    Record<string, ContainerToolPanel | undefined>
  >({});
  const [containerTerminalWidths, setContainerTerminalWidths] = useState<Record<string, number>>(
    {},
  );
  const [activeResize, setActiveResize] = useState<ResizeSession | null>(null);
  const [browserFrameVersions, setBrowserFrameVersions] = useState<Record<string, number>>({});
  const [tunnelStatuses, setTunnelStatuses] = useState<Record<string, TunnelStatus>>({});
  const [error, setError] = useState<string | null>(null);
  const overviewCardElements = useMemo(() => new Map<string, HTMLElement>(), []);
  const [overviewZoom, setOverviewZoom] = useState(() => {
    if (typeof window === "undefined") {
      return OVERVIEW_ZOOM_DEFAULT;
    }

    const stored = window.localStorage.getItem(OVERVIEW_ZOOM_STORAGE_KEY);
    const parsed = stored ? Number(stored) : Number.NaN;

    if (!Number.isFinite(parsed)) {
      return OVERVIEW_ZOOM_DEFAULT;
    }

    return clampOverviewZoom(parsed);
  });
  useEffect(() => {
    let cancelled = false;

    loadBootstrap()
      .then((nextBootstrap) => {
        if (cancelled) {
          return;
        }

        const initialTarget = resolvePreferredTarget(nextBootstrap);

        setBootstrap(nextBootstrap);
        setActiveView({ kind: "overview" });
        setVisitedContainerIds([]);
        setVisitedServerIds([]);
        setOverviewFocusedTargetId(initialTarget?.id ?? "");
        setExpandedServerIds(nextBootstrap.servers.map((server) => server.id));
        setContainerTools({});
        setContainerTerminalWidths({});
        setBrowserFrameVersions({});
        setTunnelStatuses({});
        setError(null);
      })
      .catch((loadError) => {
        if (cancelled) {
          return;
        }

        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load target definitions.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    window.localStorage.setItem(
      OVERVIEW_ZOOM_STORAGE_KEY,
      String(roundOverviewZoom(overviewZoom)),
    );
  }, [overviewZoom]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    window.localStorage.setItem(
      SIDEBAR_VISIBILITY_STORAGE_KEY,
      String(isSidebarHidden),
    );
  }, [isSidebarHidden]);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listenSidebarToggleRequested(() => {
      if (cancelled) {
        return;
      }

      setIsSidebarHidden((current) => !current);
    }).then((nextUnlisten) => {
      if (cancelled) {
        nextUnlisten();
        return;
      }

      unlisten = nextUnlisten;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (isTauri()) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        !isPlatformModifier(event) ||
        event.altKey ||
        event.shiftKey ||
        event.code !== "KeyH"
      ) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      setIsSidebarHidden((current) => !current);
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listenTunnelStatus((status) => {
      if (cancelled) {
        return;
      }

      setTunnelStatuses((current) => ({
        ...current,
        [surfaceKey(status.targetId, status.surfaceId)]: status,
      }));
    }).then((nextUnlisten) => {
      if (cancelled) {
        nextUnlisten();
        return;
      }

      unlisten = nextUnlisten;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

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
  }, [activeResize]);

  const targetById = useMemo(
    () => new Map(bootstrap?.targets.map((target) => [target.id, target]) ?? []),
    [bootstrap],
  );

  const managedTargetIds = useMemo(
    () =>
      new Set(
        bootstrap?.servers.flatMap((server) =>
          server.containers.map((container) => container.targetId),
        ) ?? [],
      ),
    [bootstrap],
  );

  const standaloneTargets = useMemo(
    () =>
      bootstrap?.targets.filter((target) => !managedTargetIds.has(target.id)) ?? [],
    [bootstrap, managedTargetIds],
  );

  const overviewContainers = useMemo(
    () =>
      bootstrap?.servers.flatMap((server) =>
        server.containers.map((container) => ({
          container,
          server,
          target: targetById.get(container.targetId),
        })),
      ) ?? [],
    [bootstrap, targetById],
  );

  const overviewTerminalTargets = useMemo(
    () => [
      ...overviewContainers
        .map((entry) => entry.target)
        .filter((target): target is DeveloperTarget => Boolean(target)),
      ...standaloneTargets,
    ],
    [overviewContainers, standaloneTargets],
  );

  const visitedServers = useMemo(
    () =>
      visitedServerIds
        .map((serverId) => bootstrap?.servers.find((server) => server.id === serverId))
        .filter((server): server is ManagedServer => Boolean(server)),
    [bootstrap, visitedServerIds],
  );

  const activeContainerTarget = useMemo(() => {
    if (!bootstrap || activeView.kind !== "container") {
      return undefined;
    }

    return bootstrap.targets.find((target) => target.id === activeView.targetId);
  }, [activeView, bootstrap]);

  const activeContainerServer = useMemo(
    () =>
      activeContainerTarget
        ? serverForTargetId(bootstrap?.servers ?? [], activeContainerTarget.id)
        : undefined,
    [activeContainerTarget, bootstrap],
  );
  const reachableContainerCount = useMemo(
    () =>
      overviewContainers.filter((entry) => entry.container.sshReachable).length,
    [overviewContainers],
  );
  const activeOverviewFocusedTargetId = useMemo(() => {
    if (overviewTerminalTargets.length === 0) {
      return "";
    }

    if (
      overviewFocusedTargetId &&
      overviewTerminalTargets.some((target) => target.id === overviewFocusedTargetId)
    ) {
      return overviewFocusedTargetId;
    }

    return overviewTerminalTargets[0]?.id ?? "";
  }, [overviewFocusedTargetId, overviewTerminalTargets]);

  useEffect(() => {
    if (activeView.kind !== "overview" || !activeOverviewFocusedTargetId) {
      return;
    }

    const element = overviewCardElements.get(activeOverviewFocusedTargetId);

    if (!element) {
      return;
    }

    element.scrollIntoView({
      behavior: "smooth",
      block: "nearest",
      inline: "nearest",
    });
  }, [activeOverviewFocusedTargetId, activeView.kind, overviewCardElements]);

  const setContainerView = (targetId: string) => {
    const nextTarget = targetById.get(targetId);

    if (!nextTarget) {
      return;
    }

    setActiveView({ kind: "container", targetId });
    setVisitedContainerIds((current) => uniqueStrings([...current, targetId]));
    setContainerTools((current) => ({
      ...current,
      [targetId]: current[targetId] ?? "files",
    }));
    setContainerTerminalWidths((current) => ({
      ...current,
      [targetId]: current[targetId] ?? defaultTerminalPanelWidthPx(),
    }));
    warmTargetTunnels(nextTarget, undefined, (status) => {
      setTunnelStatuses((current) => ({
        ...current,
        [surfaceKey(status.targetId, status.surfaceId)]: status,
      }));
    });
  };

  const openContainerViewFromShortcut = useEffectEvent((targetId: string) => {
    setContainerView(targetId);
  });

  useEffect(() => {
    if (activeView.kind !== "overview") {
      return;
    }

    const fallbackTargetId = overviewTerminalTargets[0]?.id;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isPlatformModifier(event) || event.altKey || event.shiftKey) {
        return;
      }

      const currentTargetId = activeOverviewFocusedTargetId || fallbackTargetId;

      if (event.key === "Enter") {
        if (!currentTargetId) {
          return;
        }

        event.preventDefault();
        event.stopPropagation();
        openContainerViewFromShortcut(currentTargetId);
        return;
      }

      let direction: OverviewMoveDirection | null = null;

      switch (event.key) {
        case "ArrowLeft":
          direction = "left";
          break;
        case "ArrowRight":
          direction = "right";
          break;
        case "ArrowUp":
          direction = "up";
          break;
        case "ArrowDown":
          direction = "down";
          break;
        default:
          return;
      }

      if (!direction || !currentTargetId) {
        return;
      }

      const nextTargetId = adjacentOverviewTargetId(
        direction,
        currentTargetId,
        overviewTerminalTargets.map((target) => target.id),
        overviewCardElements,
      );

      if (!nextTargetId || nextTargetId === currentTargetId) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      setOverviewFocusedTargetId(nextTargetId);
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [
    activeOverviewFocusedTargetId,
    activeView.kind,
    overviewCardElements,
    overviewTerminalTargets,
  ]);

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
        warmTargetTunnels(target, [surface.id], (status) => {
          setTunnelStatuses((current) => ({
            ...current,
            [surfaceKey(status.targetId, status.surfaceId)]: status,
          }));
        });
      }
    }
  };

  const selectContainerToolFromShortcut = useEffectEvent(
    (target: DeveloperTarget, panel: ContainerToolPanel) => {
      selectContainerTool(target, panel);
    },
  );

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
        setOverviewFocusedTargetId(targetId);
        setActiveView({ kind: "overview" });
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
  }, [activeView, targetById]);

  const startContainerResize = (
    event: ReactPointerEvent<HTMLDivElement>,
    targetId: string,
  ) => {
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
  };

  const reloadBrowserFrame = (targetId: string, surfaceId: string) => {
    const key = surfaceKey(targetId, surfaceId);

    setBrowserFrameVersions((current) => ({
      ...current,
      [key]: (current[key] ?? 0) + 1,
    }));
  };

  const restartBrowserTunnel = (targetId: string, surfaceId: string) => {
    void restartTunnel(targetId, surfaceId)
      .then((status) => {
        setTunnelStatuses((current) => ({
          ...current,
          [surfaceKey(status.targetId, status.surfaceId)]: status,
        }));
      })
      .catch((restartError) => {
        setTunnelStatuses((current) => ({
          ...current,
          [surfaceKey(targetId, surfaceId)]: {
            targetId,
            surfaceId,
            state: "error",
            message:
              restartError instanceof Error
                ? restartError.message
                : "Failed to restart browser tunnel.",
          },
        }));
      });
  };

  const toggleServerExpansion = (serverId: string) => {
    setExpandedServerIds((current) =>
      current.includes(serverId)
        ? current.filter((entry) => entry !== serverId)
        : [...current, serverId],
    );
  };

  const renderUnavailableTerminalCard = (container: ManagedContainer) => (
    <article className="panel terminal-card terminal-card-unavailable" key={container.targetId}>
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

  const renderTerminalCard = (
    target: DeveloperTarget,
    options?: {
      isGridFocused?: boolean;
      key?: string;
      onFocusRequest?: () => void;
      pageVisible?: boolean;
      registerElement?: (element: HTMLElement | null) => void;
      scale?: number;
      title?: string;
    },
  ) => (
    <TerminalCard
      isGridFocused={options?.isGridFocused}
      key={options?.key ?? target.id}
      onFocusRequest={options?.onFocusRequest}
      onOpenWorkspace={() => setContainerView(target.id)}
      pageVisible={options?.pageVisible}
      registerElement={options?.registerElement}
      scale={options?.scale}
      target={target}
      title={options?.title ?? target.label}
    />
  );

  const renderOverviewPage = (pageVisible: boolean) => {
    const reachableContainers = overviewContainers.filter(
      (entry) => entry.container.sshReachable,
    ).length;
    const overviewZoomStyle = {
      "--overview-terminal-grid-min-width": `${25 * overviewZoom}rem`,
      "--overview-terminal-card-height": `${32 * overviewZoom}rem`,
      "--overview-terminal-min-height": `${30 * overviewZoom}rem`,
    } as CSSProperties;

    return (
      <>
        <header className="content-header content-header-overview">
          <div className="content-header-copy">
            <h1 className="content-title content-title-overview">Overview</h1>
          </div>

          <div className="content-header-tools">
            <div className="overview-zoom-controls" aria-label="Overview zoom controls">
              <button
                className="panel-button panel-button-toolbar overview-zoom-button"
                disabled={overviewZoom <= OVERVIEW_ZOOM_MIN}
                onClick={() => {
                  setOverviewZoom((current) =>
                    roundOverviewZoom(
                      clampOverviewZoom(current - OVERVIEW_ZOOM_STEP),
                    ),
                  );
                }}
                title="Zoom out overview terminals"
                type="button"
              >
                -
              </button>

              <button
                className="content-chip content-chip-button"
                disabled={overviewZoom === OVERVIEW_ZOOM_DEFAULT}
                onClick={() => {
                  setOverviewZoom(OVERVIEW_ZOOM_DEFAULT);
                }}
                title="Reset overview zoom"
                type="button"
              >
                {Math.round(overviewZoom * 100)}%
              </button>

              <button
                className="panel-button panel-button-toolbar overview-zoom-button"
                disabled={overviewZoom >= OVERVIEW_ZOOM_MAX}
                onClick={() => {
                  setOverviewZoom((current) =>
                    roundOverviewZoom(
                      clampOverviewZoom(current + OVERVIEW_ZOOM_STEP),
                    ),
                  );
                }}
                title="Zoom in overview terminals"
                type="button"
              >
                +
              </button>
            </div>

            <div className="content-meta-row">
              <span className="content-chip">
                {bootstrap?.servers.length ?? 0} server(s)
              </span>
              <span className="content-chip">
                {overviewContainers.length} container(s)
              </span>
              <span className="content-chip">{reachableContainers} SSH ready</span>
            </div>
          </div>
        </header>

        <section className="page-section">
          <div className="terminal-grid" style={overviewZoomStyle}>
            {overviewContainers.map(({ container, target }) =>
              target
                ? renderTerminalCard(target, {
                    isGridFocused: activeOverviewFocusedTargetId === target.id,
                    key: container.targetId,
                    onFocusRequest: () => {
                      setOverviewFocusedTargetId(target.id);
                    },
                    pageVisible,
                    registerElement: (element) => {
                      if (element) {
                        overviewCardElements.set(target.id, element);
                      } else {
                        overviewCardElements.delete(target.id);
                      }
                    },
                    scale: overviewZoom,
                    title: container.label,
                  })
                : renderUnavailableTerminalCard(container),
            )}

            {standaloneTargets.map((target) =>
              renderTerminalCard(target, {
                isGridFocused: activeOverviewFocusedTargetId === target.id,
                key: target.id,
                onFocusRequest: () => {
                  setOverviewFocusedTargetId(target.id);
                },
                pageVisible,
                    registerElement: (element) => {
                      if (element) {
                        overviewCardElements.set(target.id, element);
                      } else {
                        overviewCardElements.delete(target.id);
                      }
                    },
                scale: overviewZoom,
                title: target.label,
              }),
            )}
          </div>
        </section>
      </>
    );
  };

  const renderServerPage = (server: ManagedServer, pageVisible: boolean) => {
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
            <span className="content-chip">
              {server.containers.length} container(s)
            </span>
            <span className="content-chip">{reachableContainers} SSH ready</span>
          </div>
        </header>

        <section className="page-section">
          <div className="terminal-grid">
            {server.containers.map((container) => {
              const target = targetById.get(container.targetId);

              return target
                ? renderTerminalCard(target, {
                    key: container.targetId,
                    pageVisible,
                    title: container.label,
                  })
                : renderUnavailableTerminalCard(container);
            })}
          </div>
        </section>
      </>
    );
  };

  const renderContainerPage = (target: DeveloperTarget, pageVisible: boolean) => {
    const activeTool = containerTools[target.id] ?? "files";
    const developmentSurface = target.surfaces.find(
      (surface) => surface.id === "development",
    );
    const previewSurface = target.surfaces.find((surface) => surface.id === "preview");
    const developmentPanel = developmentSurface
      ? (`browser:${developmentSurface.id}` as const)
      : null;
    const previewPanel = previewSurface ? (`browser:${previewSurface.id}` as const) : null;
    const activeBrowserSurfaceId = browserSurfaceIdFromPanel(activeTool);
    const browserSurface = activeBrowserSurfaceId
      ? target.surfaces.find((surface) => surface.id === activeBrowserSurfaceId)
      : undefined;
    const browserStatus = browserSurface
      ? tunnelStatuses[surfaceKey(target.id, browserSurface.id)] ??
        fallbackTunnelStatus(target.id, browserSurface)
      : undefined;
    const browserFrame: BrowserFrameInstance | undefined =
      browserSurface && browserStatus
        ? {
            cacheKey: surfaceKey(target.id, browserSurface.id),
            frameVersion:
              browserFrameVersions[surfaceKey(target.id, browserSurface.id)] ?? 0,
            isActive: pageVisible,
            status: browserStatus,
            surface: browserSurface,
            target,
          }
        : undefined;
    const workspaceStyle = {
      "--container-terminal-width": `${containerTerminalWidths[target.id] ?? defaultTerminalPanelWidthPx()}px`,
    } as CSSProperties;

    return (
      <>
        <header className="content-header content-header-container">
          <div className="content-header-copy">
            <h1 className="content-title content-title-container">{target.label}</h1>
          </div>

          <div className="content-header-tools content-header-tools-container">
            <button
              className={`panel-button panel-button-toolbar ${activeTool === "files" ? "is-active" : ""}`}
              onClick={() => {
                selectContainerTool(target, "files");
              }}
              type="button"
            >
              Files
            </button>
            <button
              className={`panel-button panel-button-toolbar ${activeTool === developmentPanel ? "is-active" : ""}`}
              disabled={!developmentSurface}
              onClick={() => {
                if (developmentPanel) {
                  selectContainerTool(target, developmentPanel);
                }
              }}
              type="button"
            >
              Dev
            </button>
            <button
              className={`panel-button panel-button-toolbar ${activeTool === previewPanel ? "is-active" : ""}`}
              disabled={!previewSurface}
              onClick={() => {
                if (previewPanel) {
                  selectContainerTool(target, previewPanel);
                }
              }}
              type="button"
            >
              Pre
            </button>
          </div>
        </header>

        <section className="page-section">
          <div className={`container-workspace ${isBrowserToolPanel(activeTool) ? "tool-browser" : "tool-files"}`} style={workspaceStyle}>
            <article className="panel terminal-panel">
              <TerminalPane active={pageVisible} target={target} />
            </article>

            <div
              className="container-resizer"
              onPointerDown={(event) => {
                startContainerResize(event, target.id);
              }}
              role="separator"
              aria-label="Resize terminal and tool panels"
              aria-orientation="vertical"
            />

            {activeTool === "files" ? (
              <FilesPane active={pageVisible} target={target} />
            ) : null}

            {isBrowserToolPanel(activeTool) && pageVisible ? (
              <BrowserPane
                activeFrame={browserFrame}
                onReload={() => {
                  if (browserSurface) {
                    reloadBrowserFrame(target.id, browserSurface.id);
                  }
                }}
                onRestart={() => {
                  if (browserSurface) {
                    restartBrowserTunnel(target.id, browserSurface.id);
                  }
                }}
                retainedFrames={browserFrame ? [browserFrame] : []}
                slotLabel={browserSurface?.label ?? "Browser"}
              />
            ) : null}
          </div>
        </section>
      </>
    );
  };

  if (error) {
    return (
      <main className="shell shell-state">
        <section className="state-panel">
          <span className="state-label">Error</span>
          <p className="state-copy">{error}</p>
        </section>
      </main>
    );
  }

  if (!bootstrap) {
    return (
      <main className="shell shell-state">
        <section className="state-panel">
          <span className="state-label">Loading</span>
          <p className="state-copy">Preparing target definitions.</p>
        </section>
      </main>
    );
  }

  const isOverviewView = activeView.kind === "overview";
  const activeStatusLabel =
    activeView.kind === "overview"
      ? "Overview"
      : activeView.kind === "server"
        ? `Server ${visitedServers.find((server) => server.id === activeView.serverId)?.label ?? activeView.serverId}`
        : `Container ${activeContainerTarget?.label ?? activeView.targetId}`;

  return (
    <main className={`shell ${isOverviewView ? "shell-overview" : ""}`}>
      <section
        className={`shell-layout ${isOverviewView ? "shell-layout-overview" : ""} ${isSidebarHidden ? "shell-layout-sidebar-hidden" : ""}`}
      >
        {!isSidebarHidden ? (
        <SidebarNav
          activeServerId={
            activeView.kind === "server"
              ? activeView.serverId
              : activeView.kind === "container"
                  ? activeContainerServer?.id
                  : undefined
            }
            activeTargetId={activeView.kind === "container" ? activeView.targetId : undefined}
            availableTargetIds={bootstrap.targets.map((target) => target.id)}
          bootstrap={bootstrap}
          expandedServerIds={expandedServerIds}
          isOverviewActive={activeView.kind === "overview"}
          isOverviewLayout={isOverviewView}
          onSelectContainer={setContainerView}
          onSelectOverview={() => {
            setActiveView({ kind: "overview" });
            }}
            onSelectServer={(serverId) => {
              setActiveView({ kind: "server", serverId });
              setVisitedServerIds((current) => uniqueStrings([...current, serverId]));
              setExpandedServerIds((current) =>
                current.includes(serverId) ? current : [...current, serverId],
              );
            }}
            onToggleServer={toggleServerExpansion}
          />
        ) : null}

        <section className={`content-shell ${isOverviewView ? "content-shell-overview" : ""}`}>
          <section
            className={`content-page content-page-overview ${activeView.kind === "overview" ? "" : "is-hidden"}`}
          >
            {renderOverviewPage(activeView.kind === "overview")}
          </section>

          {visitedServers.map((server) => (
            <section
              key={server.id}
              className={`content-page ${activeView.kind === "server" && activeView.serverId === server.id ? "" : "is-hidden"}`}
            >
              {renderServerPage(
                server,
                activeView.kind === "server" && activeView.serverId === server.id,
              )}
            </section>
          ))}

          {visitedContainerIds.map((targetId) => {
            const target = targetById.get(targetId);
            const isVisible = activeView.kind === "container" && activeView.targetId === targetId;

            return (
              <section
                className={`content-page ${isVisible ? "" : "is-hidden"}`}
                key={targetId}
              >
                {target ? (
                  renderContainerPage(target, isVisible)
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
        </section>
      </section>

      <footer className="status-bar" role="status">
        <div className="status-bar-section">
          <button
            aria-label={isSidebarHidden ? "Show sidebar menu" : "Hide sidebar menu"}
            className="panel-button panel-button-toolbar panel-button-icon status-bar-toggle"
            onClick={() => {
              setIsSidebarHidden((current) => !current);
            }}
            title={isSidebarHidden ? "Show sidebar menu" : "Hide sidebar menu"}
            type="button"
          >
            <svg
              aria-hidden="true"
              className="panel-button-icon-svg"
              fill="none"
              viewBox="0 0 16 16"
            >
              {isSidebarHidden ? (
                <path
                  d="M2.75 3.25h10.5M2.75 8h10.5M2.75 12.75h10.5"
                  stroke="currentColor"
                  strokeLinecap="round"
                  strokeWidth="1.25"
                />
              ) : (
                <>
                  <path
                    d="M3.25 3.25v9.5M6.25 4h6.5M6.25 8h6.5M6.25 12h6.5"
                    stroke="currentColor"
                    strokeLinecap="round"
                    strokeWidth="1.25"
                  />
                  <path
                    d="M3.25 8h1.5"
                    stroke="currentColor"
                    strokeLinecap="round"
                    strokeWidth="1.25"
                  />
                </>
              )}
            </svg>
          </button>
          <span className="status-bar-chip">{activeStatusLabel}</span>
          <span className="status-bar-text">
            Inventory ready
          </span>
        </div>

        <div className="status-bar-section">
          <span className="status-bar-text">
            {bootstrap.servers.length} server(s)
          </span>
          <span className="status-bar-text">
            {overviewContainers.length} container(s)
          </span>
          <span className="status-bar-text">
            {reachableContainerCount} SSH ready
          </span>
          {isOverviewView ? (
            <span className="status-bar-text">
              Overview zoom {Math.round(overviewZoom * 100)}%
            </span>
          ) : null}
        </div>
      </footer>
    </main>
  );
}

export default App;
