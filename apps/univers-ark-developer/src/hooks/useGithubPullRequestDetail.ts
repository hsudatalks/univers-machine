import { useCallback, useEffect, useState } from "react";
import {
  loadGithubPullRequestDetail,
  mergeGithubPullRequest,
} from "../lib/tauri";
import type { GithubMergeMethod, GithubPullRequestDetail } from "../types";

type UseGithubPullRequestDetailResult = {
  detail: GithubPullRequestDetail | null;
  error: string;
  isLoading: boolean;
  isMerging: boolean;
  mergeError: string;
  mergeSuccessMessage: string;
  mergePullRequest: (method: GithubMergeMethod) => Promise<boolean>;
  refresh: () => Promise<void>;
};

export function useGithubPullRequestDetail(
  isOpen: boolean,
  selectedNumber: number | null,
): UseGithubPullRequestDetailResult {
  const [detail, setDetail] = useState<GithubPullRequestDetail | null>(null);
  const [error, setError] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isMerging, setIsMerging] = useState(false);
  const [mergeError, setMergeError] = useState("");
  const [mergeSuccessMessage, setMergeSuccessMessage] = useState("");

  const refresh = useCallback(async () => {
    if (!selectedNumber) {
      setDetail(null);
      setError("");
      return;
    }

    setIsLoading(true);
    setError("");

    try {
      const nextDetail = await loadGithubPullRequestDetail(selectedNumber);
      setDetail(nextDetail);
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Failed to load pull request details.",
      );
    } finally {
      setIsLoading(false);
    }
  }, [selectedNumber]);

  useEffect(() => {
    if (!isOpen || !selectedNumber) {
      return;
    }

    void refresh();
  }, [isOpen, refresh, selectedNumber]);

  const mergePullRequest = useCallback(async (method: GithubMergeMethod) => {
    if (!selectedNumber) {
      return false;
    }

    setIsMerging(true);
    setMergeError("");
    setMergeSuccessMessage("");

    try {
      await mergeGithubPullRequest(selectedNumber, method);
      setMergeSuccessMessage(
        `${method[0].toUpperCase()}${method.slice(1)} merged PR #${selectedNumber}.`,
      );
      return true;
    } catch (nextError) {
      setMergeError(
        nextError instanceof Error
          ? nextError.message
          : `Failed to merge PR #${selectedNumber}.`,
      );
      return false;
    } finally {
      setIsMerging(false);
    }
  }, [selectedNumber]);

  return {
    detail,
    error,
    isLoading,
    isMerging,
    mergeError,
    mergeSuccessMessage,
    mergePullRequest,
    refresh,
  };
}
