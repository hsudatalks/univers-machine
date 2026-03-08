import { useCallback, useEffect, useState } from "react";
import { loadGithubProjectState } from "../lib/tauri";
import type { GithubProjectState } from "../types";

type UseGithubProjectStateResult = {
  error: string;
  isLoading: boolean;
  projectState: GithubProjectState | null;
  refresh: () => Promise<void>;
};

export function useGithubProjectState(isOpen: boolean): UseGithubProjectStateResult {
  const [projectState, setProjectState] = useState<GithubProjectState | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    setIsLoading(true);
    setError("");

    try {
      const nextState = await loadGithubProjectState();
      setProjectState(nextState);
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Failed to load GitHub project state.",
      );
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!isOpen || projectState || isLoading) {
      return;
    }

    void refresh();
  }, [isLoading, isOpen, projectState, refresh]);

  return { error, isLoading, projectState, refresh };
}
