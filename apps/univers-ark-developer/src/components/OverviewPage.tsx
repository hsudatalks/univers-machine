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
  isRefreshing: boolean;
  onFocusTarget: (targetId: string) => void;
  onOpenWorkspace: (targetId: string) => void;
  onRefreshInventory: () => void;
  overviewContainers: OverviewEntry[];
  overviewZoom: number;
  overviewZoomStyle: CSSProperties;
  pageVisible: boolean;
  registerOverviewCardElement: (targetId: string, element: HTMLElement | null) => void;
  standaloneTargets: DeveloperTarget[];
}

function UnavailableTerminalCard({
  container,
  isRefreshing,
  onRetry,
}: {
  container: ManagedContainer;
  isRefreshing: boolean;
  onRetry: () => void;
}) {
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
        <button
          className="panel-button panel-button-retry"
          disabled={isRefreshing}
          onClick={onRetry}
          type="button"
        >
          {isRefreshing ? "Retrying…" : "Retry"}
        </button>
      </div>
    </article>
  );
}

export function OverviewPage({
  activeFocusedTargetId,
  isRefreshing,
  onFocusTarget,
  onOpenWorkspace,
  onRefreshInventory,
  overviewContainers,
  overviewZoom,
  overviewZoomStyle,
  pageVisible,
  registerOverviewCardElement,
  standaloneTargets,
}: OverviewPageProps) {
  return (
    <>
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
              <UnavailableTerminalCard
                container={container}
                isRefreshing={isRefreshing}
                key={container.targetId}
                onRetry={onRefreshInventory}
              />
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
