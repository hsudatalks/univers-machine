import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { openExternalLink } from "../lib/tauri";
import { useGithubProjectState } from "../hooks/useGithubProjectState";
import { useGithubPullRequestDetail } from "../hooks/useGithubPullRequestDetail";
import type { GithubMergeMethod, GithubPullRequestSummary } from "../types";

type PullRequestListFilter = "all" | "mine" | "open" | "closed" | "merged";

function formatRelativeTimestamp(timestamp: string): string {
  const date = new Date(timestamp);

  if (Number.isNaN(date.getTime())) {
    return timestamp;
  }

  const diffMs = Date.now() - date.getTime();
  const diffMinutes = Math.max(1, Math.round(diffMs / 60000));

  if (diffMinutes < 60) {
    return `${diffMinutes}m ago`;
  }

  const diffHours = Math.round(diffMinutes / 60);

  if (diffHours < 24) {
    return `${diffHours}h ago`;
  }

  const diffDays = Math.round(diffHours / 24);
  return `${diffDays}d ago`;
}

function statusTone(status: string): string {
  const normalized = status.toUpperCase();

  if (normalized === "MERGED" || normalized === "SUCCESS" || normalized === "APPROVED") {
    return "success";
  }

  if (
    normalized === "CLOSED" ||
    normalized === "FAILURE" ||
    normalized === "BLOCKED" ||
    normalized === "CHANGES_REQUESTED"
  ) {
    return "danger";
  }

  if (normalized === "REVIEW_REQUIRED" || normalized === "PENDING") {
    return "warning";
  }

  return "neutral";
}

function PullRequestStatusBadges({ pr }: { pr: GithubPullRequestSummary }) {
  return (
    <span className="github-badge-row">
      <span className={`github-badge github-badge-${statusTone(pr.state)}`}>{pr.state}</span>
      {pr.isDraft ? <span className="github-badge github-badge-neutral">DRAFT</span> : null}
      {pr.reviewDecision ? (
        <span className={`github-badge github-badge-${statusTone(pr.reviewDecision)}`}>
          {pr.reviewDecision}
        </span>
      ) : null}
    </span>
  );
}

function summarizeMergeBlockers(detail: {
  mergeStateStatus: string;
  reviewDecision: string | null;
  statusChecks: Array<{ status: string; conclusion: string | null }>;
}) {
  const blockers: string[] = [];

  if (detail.reviewDecision === "REVIEW_REQUIRED") {
    blockers.push("review required");
  }

  if (detail.reviewDecision === "CHANGES_REQUESTED") {
    blockers.push("changes requested");
  }

  if (detail.mergeStateStatus === "BLOCKED") {
    blockers.push("merge blocked by repository rules");
  }

  const failingChecks = detail.statusChecks.filter(
    (check) => check.conclusion === "FAILURE" || check.conclusion === "TIMED_OUT",
  ).length;

  if (failingChecks > 0) {
    blockers.push(`${failingChecks} failing check${failingChecks === 1 ? "" : "s"}`);
  }

  const pendingChecks = detail.statusChecks.filter(
    (check) => check.status === "IN_PROGRESS" || check.status === "QUEUED",
  ).length;

  if (pendingChecks > 0) {
    blockers.push(`${pendingChecks} pending check${pendingChecks === 1 ? "" : "s"}`);
  }

  return blockers;
}

function summarizeChecks(statusChecks: Array<{ status: string; conclusion: string | null }>) {
  let passing = 0;
  let failing = 0;
  let pending = 0;

  for (const check of statusChecks) {
    if (check.status === "IN_PROGRESS" || check.status === "QUEUED") {
      pending += 1;
      continue;
    }

    if (check.conclusion === "SUCCESS") {
      passing += 1;
      continue;
    }

    if (
      check.conclusion === "FAILURE" ||
      check.conclusion === "TIMED_OUT" ||
      check.conclusion === "CANCELLED" ||
      check.conclusion === "STARTUP_FAILURE"
    ) {
      failing += 1;
    }
  }

  return { failing, passing, pending };
}

function PullRequestList({
  items,
  selectedNumber,
  title,
  isDetailLoading,
  onSelect,
}: {
  items: GithubPullRequestSummary[];
  selectedNumber: number | null;
  title: string;
  isDetailLoading?: boolean;
  onSelect: (pr: GithubPullRequestSummary) => void;
}) {
  return (
    <section className="github-section">
      <div className="github-section-header">
        <span className="github-section-title">{title}</span>
        <span className="github-section-count">{items.length}</span>
      </div>
      {items.length ? (
        <ul className="github-pr-list">
          {items.map((pr) => (
            <li className="github-pr-item" key={pr.number}>
              <button
                className={`github-pr-button ${selectedNumber === pr.number ? "is-selected" : ""}`}
                disabled={selectedNumber === pr.number && isDetailLoading}
                onClick={() => onSelect(pr)}
                type="button"
              >
                <span className="github-pr-title">
                  #{pr.number} {pr.title}
                </span>
                <PullRequestStatusBadges pr={pr} />
                <span className="github-pr-meta">
                  {pr.headRefName} · {pr.authorLogin} ·{" "}
                  {formatRelativeTimestamp(pr.updatedAt)}
                </span>
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="github-empty">No pull requests to show.</p>
      )}
    </section>
  );
}

function matchesSearchQuery(pr: GithubPullRequestSummary, query: string): boolean {
  if (!query.trim()) {
    return true;
  }

  const normalized = query.trim().toLowerCase();
  return (
    pr.title.toLowerCase().includes(normalized) ||
    pr.headRefName.toLowerCase().includes(normalized) ||
    pr.authorLogin.toLowerCase().includes(normalized) ||
    String(pr.number).includes(normalized)
  );
}

export function GithubPopover() {
  const [isOpen, setIsOpen] = useState(false);
  const [selectedNumber, setSelectedNumber] = useState<number | null>(null);
  const [mergeArmedNumber, setMergeArmedNumber] = useState<number | null>(null);
  const [mergeMethod, setMergeMethod] = useState<GithubMergeMethod>("merge");
  const [expandedReviewKeys, setExpandedReviewKeys] = useState<string[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [listFilter, setListFilter] = useState<PullRequestListFilter>("open");
  const { error, isLoading, projectState, refresh } = useGithubProjectState(isOpen);
  const rootRef = useRef<HTMLDivElement | null>(null);

  const filteredSections = useMemo(() => {
    if (!projectState) {
      return {
        closedPrs: [],
        mergedPrs: [],
        myOpenPrs: [],
        openPrs: [],
      };
    }

    const filterItems = (items: GithubPullRequestSummary[]) =>
      items.filter((pullRequest) => matchesSearchQuery(pullRequest, searchQuery));

    return {
      closedPrs: filterItems(projectState.closedPrs),
      mergedPrs: filterItems(projectState.mergedPrs),
      myOpenPrs: filterItems(projectState.myOpenPrs),
      openPrs: filterItems(projectState.openPrs),
    };
  }, [projectState, searchQuery]);

  const visiblePullRequests = useMemo(() => {
    if (!projectState) {
      return [];
    }

    const buckets: GithubPullRequestSummary[] = [];

    if (projectState.currentBranchPr && matchesSearchQuery(projectState.currentBranchPr, searchQuery)) {
      buckets.push(projectState.currentBranchPr);
    }

    if (listFilter === "all" || listFilter === "mine") {
      buckets.push(...filteredSections.myOpenPrs);
    }

    if (listFilter === "all" || listFilter === "open") {
      buckets.push(...filteredSections.openPrs);
    }

    if (listFilter === "all" || listFilter === "closed") {
      buckets.push(...filteredSections.closedPrs);
    }

    if (listFilter === "all" || listFilter === "merged") {
      buckets.push(...filteredSections.mergedPrs);
    }

    const deduped = new Map<number, GithubPullRequestSummary>();

    for (const pr of buckets) {
      deduped.set(pr.number, pr);
    }

    return [...deduped.values()];
  }, [filteredSections, listFilter, projectState, searchQuery]);

  const fallbackSelectedNumber =
    projectState?.currentBranchPr?.number ??
    visiblePullRequests[0]?.number ??
    null;

  const activeSelectedNumber =
    selectedNumber &&
    visiblePullRequests.some((pullRequest) => pullRequest.number === selectedNumber)
      ? selectedNumber
      : fallbackSelectedNumber;

  const mergeArmed = mergeArmedNumber === activeSelectedNumber;

  const {
    detail,
    error: detailError,
    isLoading: isDetailLoading,
    isMerging,
    mergeError,
    mergeSuccessMessage,
    mergePullRequest,
    refresh: refreshDetail,
  } = useGithubPullRequestDetail(isOpen, activeSelectedNumber);

  const checkSummary = detail ? summarizeChecks(detail.statusChecks) : null;
  const isRefreshing = isLoading || isDetailLoading;
  const mergeBlockers = detail ? summarizeMergeBlockers(detail) : [];

  const handleOpenLink = useCallback(async (url: string) => {
    try {
      await openExternalLink(url);
    } catch (nextError) {
      console.error(nextError);
    }
  }, []);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    function handlePointerDown(event: MouseEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setIsOpen(false);
      }
    }

    function handleEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setIsOpen(false);
      }
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleEscape);

    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleEscape);
    };
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    function handleNavigation(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      const tagName = target?.tagName;

      if (tagName === "INPUT" || tagName === "TEXTAREA" || target?.isContentEditable) {
        return;
      }

      if (
        event.key !== "ArrowDown" &&
        event.key !== "ArrowUp" &&
        event.key !== "Enter"
      ) {
        return;
      }

      if (!visiblePullRequests.length) {
        return;
      }

      if (event.key === "Enter") {
        const selectedPullRequest = visiblePullRequests.find(
          (pullRequest) => pullRequest.number === activeSelectedNumber,
        );

        if (selectedPullRequest) {
          event.preventDefault();
          void handleOpenLink(selectedPullRequest.url);
        }

        return;
      }

      const currentIndex = visiblePullRequests.findIndex(
        (pullRequest) => pullRequest.number === activeSelectedNumber,
      );
      const baseIndex = currentIndex === -1 ? 0 : currentIndex;
      const nextIndex =
        event.key === "ArrowDown"
          ? Math.min(visiblePullRequests.length - 1, baseIndex + 1)
          : Math.max(0, baseIndex - 1);

      const nextPullRequest = visiblePullRequests[nextIndex];

      if (nextPullRequest) {
        event.preventDefault();
        setSelectedNumber(nextPullRequest.number);
        setMergeArmedNumber(null);
      }
    }

    document.addEventListener("keydown", handleNavigation);

    return () => {
      document.removeEventListener("keydown", handleNavigation);
    };
  }, [activeSelectedNumber, handleOpenLink, isOpen, visiblePullRequests]);

  const handleRefresh = async () => {
    await Promise.all([refresh(), refreshDetail()]);
  };

  const handleMerge = async () => {
    if (!activeSelectedNumber) {
      return;
    }

    if (!mergeArmed) {
      setMergeArmedNumber(activeSelectedNumber);
      return;
    }

    const merged = await mergePullRequest(mergeMethod);

    if (merged) {
      setMergeArmedNumber(null);
      await refresh();
      await refreshDetail();
    }
  };

  const toggleReviewExpanded = (key: string) => {
    setExpandedReviewKeys((current) =>
      current.includes(key)
        ? current.filter((entry) => entry !== key)
        : [...current, key],
    );
  };

  return (
    <div className="github-popover-root" ref={rootRef}>
      <button
        aria-expanded={isOpen}
        aria-haspopup="dialog"
        className={`panel-button panel-button-toolbar panel-button-icon ${isOpen ? "is-active" : ""}`}
        onClick={() => setIsOpen((current) => !current)}
        title="Manage hvac-workbench pull requests"
        type="button"
      >
        <svg
          aria-hidden="true"
          className="panel-button-icon-svg"
          fill="currentColor"
          viewBox="0 0 16 16"
        >
          <path d="M8 0C3.58 0 0 3.73 0 8.33c0 3.68 2.29 6.79 5.47 7.89.4.08.55-.18.55-.4 0-.2-.01-.86-.01-1.56-2.01.45-2.53-.51-2.69-.98-.09-.24-.48-.98-.82-1.18-.28-.16-.68-.57-.01-.58.63-.01 1.08.59 1.23.83.72 1.26 1.87.9 2.33.69.07-.54.28-.9.51-1.11-1.78-.21-3.64-.92-3.64-4.08 0-.9.31-1.64.82-2.22-.08-.21-.36-1.05.08-2.19 0 0 .67-.22 2.2.85A7.36 7.36 0 0 1 8 4.9c.68 0 1.37.09 2.01.27 1.53-1.08 2.2-.85 2.2-.85.44 1.14.16 1.98.08 2.19.51.58.82 1.31.82 2.22 0 3.17-1.87 3.87-3.65 4.08.29.26.54.77.54 1.57 0 1.13-.01 2.03-.01 2.31 0 .22.14.49.55.4A8.33 8.33 0 0 0 16 8.33C16 3.73 12.42 0 8 0Z" />
        </svg>
      </button>

      {isOpen ? (
        <div
          aria-label="GitHub hvac-workbench pull request panel"
          className="github-popover panel"
          role="dialog"
        >
          <div className="panel-header github-popover-header">
            <div className="github-popover-copy">
              <span className="panel-title">GitHub</span>
              <p className="panel-description github-popover-description">
                hvac-workbench pull requests via local <code>gh</code>
              </p>
            </div>
            <div className="content-header-tools">
              <button
                className="panel-button panel-button-toolbar"
                disabled={isRefreshing}
                onClick={() => void handleRefresh()}
                type="button"
              >
                {isRefreshing ? "Refreshing…" : "Refresh"}
              </button>
            </div>
          </div>

          <div className="github-popover-body github-popover-layout">
            <aside className="github-sidebar">
              {isLoading && !projectState ? (
                <p className="github-empty">Loading GitHub project state…</p>
              ) : null}

              {error ? <p className="github-error">{error}</p> : null}

              {projectState ? (
                <>
                  <section className="github-section github-section-summary">
                    <div className="github-repo-row">
                      <button
                        className="github-repo-link"
                        onClick={() => void handleOpenLink(projectState.repository.url)}
                        type="button"
                      >
                        {projectState.repository.nameWithOwner}
                      </button>
                      <span className="content-chip">
                        {projectState.repository.defaultBranch}
                      </span>
                    </div>
                    {projectState.repository.localBranch ? (
                      <p className="github-summary-line">
                        Local branch {projectState.repository.localBranch}
                      </p>
                    ) : null}
                    {projectState.repository.localStatusSummary ? (
                      <p className="github-summary-line">
                        {projectState.repository.localStatusSummary}
                      </p>
                    ) : null}
                    {projectState.currentBranchPr ? (
                      <button
                        className="github-current-pr"
                        onClick={() => {
                          setSelectedNumber(projectState.currentBranchPr!.number);
                          setMergeArmedNumber(null);
                        }}
                        type="button"
                      >
                        Current branch PR #{projectState.currentBranchPr.number}:{" "}
                        {projectState.currentBranchPr.title}
                      </button>
                    ) : (
                      <p className="github-empty">
                        Current local branch has no linked pull request.
                      </p>
                    )}
                  </section>

                  <section className="github-section">
                    <div className="github-filter-bar" role="tablist" aria-label="Pull request lists">
                      {(
                        [
                          ["open", "Open"],
                          ["mine", "Mine"],
                          ["closed", "Closed"],
                          ["merged", "Merged"],
                          ["all", "All"],
                        ] as Array<[PullRequestListFilter, string]>
                      ).map(([value, label]) => (
                        <button
                          aria-selected={listFilter === value}
                          className={`panel-button panel-button-toolbar github-filter-button ${listFilter === value ? "is-active" : ""}`}
                          key={value}
                          onClick={() => setListFilter(value)}
                          role="tab"
                          type="button"
                        >
                          {label}
                        </button>
                      ))}
                    </div>
                    <label className="github-search-field">
                      <span className="github-search-label">Search</span>
                      <input
                        className="github-search-input"
                        onChange={(event) => setSearchQuery(event.target.value)}
                        placeholder="Title, branch, author, #"
                        type="search"
                        value={searchQuery}
                      />
                    </label>
                  </section>

                  {listFilter === "all" || listFilter === "mine" ? (
                    <PullRequestList
                      items={filteredSections.myOpenPrs}
                      onSelect={(pr) => {
                        setSelectedNumber(pr.number);
                        setMergeArmedNumber(null);
                      }}
                      selectedNumber={activeSelectedNumber}
                      isDetailLoading={isDetailLoading}
                      title="My open PRs"
                    />
                  ) : null}

                  {listFilter === "all" || listFilter === "open" ? (
                    <PullRequestList
                      items={filteredSections.openPrs}
                      onSelect={(pr) => {
                        setSelectedNumber(pr.number);
                        setMergeArmedNumber(null);
                      }}
                      selectedNumber={activeSelectedNumber}
                      isDetailLoading={isDetailLoading}
                      title="Open PRs"
                    />
                  ) : null}

                  {listFilter === "all" || listFilter === "closed" ? (
                    <PullRequestList
                      items={filteredSections.closedPrs}
                      onSelect={(pr) => {
                        setSelectedNumber(pr.number);
                        setMergeArmedNumber(null);
                      }}
                      selectedNumber={activeSelectedNumber}
                      isDetailLoading={isDetailLoading}
                      title="Closed PRs"
                    />
                  ) : null}

                  {listFilter === "all" || listFilter === "merged" ? (
                    <PullRequestList
                      items={filteredSections.mergedPrs}
                      onSelect={(pr) => {
                        setSelectedNumber(pr.number);
                        setMergeArmedNumber(null);
                      }}
                      selectedNumber={activeSelectedNumber}
                      isDetailLoading={isDetailLoading}
                      title="Merged PRs"
                    />
                  ) : null}
                </>
              ) : null}
            </aside>

            <section className="github-detail-panel">
              {isDetailLoading ? (
                <p className="github-empty">Loading pull request details…</p>
              ) : null}

              {detailError ? <p className="github-error">{detailError}</p> : null}

              {detail ? (
                <>
                  <div className="github-detail-header">
                    <div className="github-detail-copy">
                      <button
                        className="github-detail-link"
                        onClick={() => void handleOpenLink(detail.url)}
                        type="button"
                      >
                        #{detail.number} {detail.title}
                      </button>
                      <p className="github-pr-meta">
                        {detail.headRefName} → {detail.baseRefName} · {detail.authorLogin} ·{" "}
                        {formatRelativeTimestamp(detail.updatedAt)}
                      </p>
                      <div className="github-badge-row">
                        <span className={`github-badge github-badge-${statusTone(detail.state)}`}>
                          {detail.state}
                        </span>
                        {detail.isDraft ? (
                          <span className="github-badge github-badge-neutral">DRAFT</span>
                        ) : null}
                        <span
                          className={`github-badge github-badge-${statusTone(detail.mergeStateStatus)}`}
                        >
                          {detail.mergeStateStatus}
                        </span>
                        <span
                          className={`github-badge github-badge-${statusTone(detail.mergeable)}`}
                        >
                          {detail.mergeable}
                        </span>
                        {detail.reviewDecision ? (
                          <span
                            className={`github-badge github-badge-${statusTone(detail.reviewDecision)}`}
                          >
                            {detail.reviewDecision}
                          </span>
                        ) : null}
                      </div>
                    </div>
                    <div className="github-detail-actions">
                      <div className="github-merge-methods" role="tablist" aria-label="Merge method">
                        {(["merge", "squash", "rebase"] as GithubMergeMethod[]).map((method) => (
                          <button
                            key={method}
                            className={`panel-button panel-button-toolbar ${mergeMethod === method ? "is-active" : ""}`}
                            disabled={isMerging}
                            onClick={() => setMergeMethod(method)}
                            type="button"
                          >
                            {method}
                          </button>
                        ))}
                      </div>
                      <button
                        className={`panel-button panel-button-toolbar ${mergeArmed ? "is-active" : ""}`}
                        disabled={isMerging || detail.state !== "OPEN"}
                        onClick={() => void handleMerge()}
                        type="button"
                      >
                        {isMerging
                          ? "Merging…"
                          : mergeArmed
                            ? `Confirm ${mergeMethod} #${detail.number}`
                            : "Merge"}
                      </button>
                    </div>
                  </div>

                  {mergeError ? <p className="github-error">{mergeError}</p> : null}
                  {mergeSuccessMessage ? (
                    <p className="github-success">{mergeSuccessMessage}</p>
                  ) : null}

                  <p className="github-summary-line">
                    Merge method <strong>{mergeMethod}</strong> into{" "}
                    <strong>{detail.baseRefName}</strong>
                    {mergeArmed ? " · click merge again to confirm" : ""}
                  </p>

                  <div className="github-detail-metrics">
                    <span className="content-chip">{detail.changedFiles} files</span>
                    <span className="content-chip">+{detail.additions}</span>
                    <span className="content-chip">-{detail.deletions}</span>
                    {checkSummary ? (
                      <>
                        <span className="content-chip">{checkSummary.passing} pass</span>
                        <span className="content-chip">{checkSummary.pending} pending</span>
                        <span className="content-chip">{checkSummary.failing} fail</span>
                      </>
                    ) : null}
                  </div>

                  {mergeBlockers.length ? (
                    <section className="github-detail-section">
                      <div className="github-section-header">
                        <span className="github-section-title">Merge blockers</span>
                      </div>
                      <ul className="github-inline-list">
                        {mergeBlockers.map((blocker) => (
                          <li className="github-inline-item" key={blocker}>
                            {blocker}
                          </li>
                        ))}
                      </ul>
                    </section>
                  ) : null}

                  <section className="github-detail-section">
                    <div className="github-section-header">
                      <span className="github-section-title">Body</span>
                    </div>
                    {detail.body.trim() ? (
                      <div className="github-markdown">
                        <ReactMarkdown
                          remarkPlugins={[remarkGfm]}
                          components={{
                            a: ({ children, href }) => (
                              <button
                                className="github-inline-link github-markdown-link"
                                onClick={() => (href ? void handleOpenLink(href) : undefined)}
                                type="button"
                              >
                                {children}
                              </button>
                            ),
                          }}
                        >
                          {detail.body}
                        </ReactMarkdown>
                      </div>
                    ) : (
                      <p className="github-empty">No body content.</p>
                    )}
                  </section>

                  <section className="github-detail-section">
                    <div className="github-section-header">
                      <span className="github-section-title">Files</span>
                      <span className="github-section-count">{detail.files.length}</span>
                    </div>
                    {detail.files.length ? (
                      <ul className="github-file-list">
                        {detail.files.map((file) => (
                          <li className="github-file-item" key={file.path}>
                            <span className="github-file-path">{file.path}</span>
                            <span className="github-file-stats">
                              +{file.additions} / -{file.deletions}
                            </span>
                          </li>
                        ))}
                      </ul>
                    ) : (
                      <p className="github-empty">No file list available.</p>
                    )}
                  </section>

                  <section className="github-detail-section">
                    <div className="github-section-header">
                      <span className="github-section-title">Checks</span>
                      <span className="github-section-count">{detail.statusChecks.length}</span>
                    </div>
                    {detail.statusChecks.length ? (
                      <ul className="github-review-list">
                        {detail.statusChecks.map((check, index) => (
                          <li
                            className="github-review-item"
                            key={`${check.name}-${check.status}-${index}`}
                          >
                            <div className="github-review-header">
                              <span className="github-review-author">{check.name}</span>
                              <span
                                className={`github-badge github-badge-${statusTone(check.conclusion || check.status)}`}
                              >
                                {check.conclusion || check.status}
                              </span>
                            </div>
                            {check.workflowName ? (
                              <p className="github-pr-meta">{check.workflowName}</p>
                            ) : null}
                            {check.detailsUrl ? (
                              <button
                                className="github-inline-link"
                                onClick={() => void handleOpenLink(check.detailsUrl)}
                                type="button"
                              >
                                Open check run
                              </button>
                            ) : null}
                          </li>
                        ))}
                      </ul>
                    ) : (
                      <p className="github-empty">No checks reported.</p>
                    )}
                  </section>

                  <section className="github-detail-section">
                    <div className="github-section-header">
                      <span className="github-section-title">Latest reviews</span>
                      <span className="github-section-count">
                        {detail.latestReviews.length}
                      </span>
                    </div>
                    {detail.latestReviews.length ? (
                      <ul className="github-review-list">
                        {detail.latestReviews.map((review, index) => (
                          <li
                            className="github-review-item"
                            key={`${review.authorLogin}-${review.submittedAt}-${index}`}
                          >
                            <div className="github-review-header">
                              <span className="github-review-author">
                                {review.authorLogin}
                              </span>
                              <span
                                className={`github-badge github-badge-${statusTone(review.state)}`}
                              >
                                {review.state}
                              </span>
                            </div>
                            {review.body ? (
                              <>
                                <p className={`github-review-body ${expandedReviewKeys.includes(`${review.authorLogin}-${review.submittedAt}-${index}`) ? "is-expanded" : "is-collapsed"}`}>
                                  {review.body}
                                </p>
                                {review.body.length > 240 ? (
                                  <button
                                    className="github-inline-link"
                                    onClick={() =>
                                      toggleReviewExpanded(
                                        `${review.authorLogin}-${review.submittedAt}-${index}`,
                                      )
                                    }
                                    type="button"
                                  >
                                    {expandedReviewKeys.includes(
                                      `${review.authorLogin}-${review.submittedAt}-${index}`,
                                    )
                                      ? "Collapse"
                                      : "Expand"}
                                  </button>
                                ) : null}
                              </>
                            ) : null}
                            <p className="github-pr-meta">
                              {formatRelativeTimestamp(review.submittedAt)}
                            </p>
                          </li>
                        ))}
                      </ul>
                    ) : (
                      <p className="github-empty">No review activity yet.</p>
                    )}
                  </section>
                </>
              ) : !isDetailLoading && !detailError ? (
                <p className="github-empty">
                  Select a pull request to inspect its contents.
                </p>
              ) : null}
            </section>
          </div>
        </div>
      ) : null}
    </div>
  );
}
