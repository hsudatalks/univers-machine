import type { CSSProperties } from "react";
import { TerminalCard } from "./TerminalCard";
import type { DeveloperTarget, ManagedContainer, ManagedServer } from "../types";

interface OverviewEntry {
  container: ManagedContainer;
  server: ManagedServer;
  target?: DeveloperTarget;
}

interface OverviewPageProps {
  activeFocusedTargetId: string;
  onFocusTarget: (targetId: string) => void;
  onOpenWorkspace: (targetId: string) => void;
  onResetZoom: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  overviewContainers: OverviewEntry[];
  overviewZoom: number;
  overviewZoomDefault: number;
  overviewZoomMax: number;
  overviewZoomMin: number;
  overviewZoomStyle: CSSProperties;
  pageVisible: boolean;
  registerOverviewCardElement: (targetId: string, element: HTMLElement | null) => void;
  serverCount: number;
  standaloneTargets: DeveloperTarget[];
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

export function OverviewPage({
  activeFocusedTargetId,
  onFocusTarget,
  onOpenWorkspace,
  onResetZoom,
  onZoomIn,
  onZoomOut,
  overviewContainers,
  overviewZoom,
  overviewZoomDefault,
  overviewZoomMax,
  overviewZoomMin,
  overviewZoomStyle,
  pageVisible,
  registerOverviewCardElement,
  serverCount,
  standaloneTargets,
}: OverviewPageProps) {
  const reachableContainers = overviewContainers.filter(
    (entry) => entry.container.sshReachable,
  ).length;

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
              disabled={overviewZoom <= overviewZoomMin}
              onClick={onZoomOut}
              title="Zoom out overview terminals"
              type="button"
            >
              -
            </button>

            <button
              className="content-chip content-chip-button"
              disabled={overviewZoom === overviewZoomDefault}
              onClick={onResetZoom}
              title="Reset overview zoom"
              type="button"
            >
              {Math.round(overviewZoom * 100)}%
            </button>

            <button
              className="panel-button panel-button-toolbar overview-zoom-button"
              disabled={overviewZoom >= overviewZoomMax}
              onClick={onZoomIn}
              title="Zoom in overview terminals"
              type="button"
            >
              +
            </button>
          </div>

          <div className="content-meta-row">
            <span className="content-chip">{serverCount} server(s)</span>
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
            target ? (
              <TerminalCard
                isGridFocused={activeFocusedTargetId === target.id}
                key={container.targetId}
                onFocusRequest={() => {
                  onFocusTarget(target.id);
                }}
                onOpenWorkspace={() => {
                  onOpenWorkspace(target.id);
                }}
                pageVisible={pageVisible}
                registerElement={(element) => {
                  registerOverviewCardElement(target.id, element);
                }}
                scale={overviewZoom}
                target={target}
                title={container.label}
              />
            ) : (
              <UnavailableTerminalCard container={container} key={container.targetId} />
            ),
          )}

          {standaloneTargets.map((target) => (
            <TerminalCard
              isGridFocused={activeFocusedTargetId === target.id}
              key={target.id}
              onFocusRequest={() => {
                onFocusTarget(target.id);
              }}
              onOpenWorkspace={() => {
                onOpenWorkspace(target.id);
              }}
              pageVisible={pageVisible}
              registerElement={(element) => {
                registerOverviewCardElement(target.id, element);
              }}
              scale={overviewZoom}
              target={target}
              title={target.label}
            />
          ))}
        </div>
      </section>
    </>
  );
}
