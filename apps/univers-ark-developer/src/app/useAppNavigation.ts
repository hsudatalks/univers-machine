import { useCallback, useEffect, useRef, useState } from "react";
import {
  hashForActiveView,
  parseActiveViewFromHash,
  sameActiveView,
  type ActiveView,
} from "../lib/view-types";

export function useAppNavigation() {
  const [activeView, setActiveView] = useState<ActiveView>(
    () => parseActiveViewFromHash(window.location.hash) ?? { kind: "home" },
  );
  const previousNonSettingsViewRef = useRef<ActiveView>(activeView);

  useEffect(() => {
    if (activeView.kind !== "settings") {
      previousNonSettingsViewRef.current = activeView;
    }
  }, [activeView]);

  useEffect(() => {
    const handleHashChange = () => {
      const nextView =
        parseActiveViewFromHash(window.location.hash) ?? { kind: "home" as const };

      setActiveView((current) =>
        sameActiveView(current, nextView) ? current : nextView,
      );
    };

    window.addEventListener("hashchange", handleHashChange);

    return () => {
      window.removeEventListener("hashchange", handleHashChange);
    };
  }, []);

  useEffect(() => {
    const nextHash = hashForActiveView(activeView);

    if (window.location.hash === nextHash) {
      return;
    }

    if (!window.location.hash) {
      window.history.replaceState(
        null,
        "",
        `${window.location.pathname}${window.location.search}${nextHash}`,
      );
      return;
    }

    window.location.hash = nextHash;
  }, [activeView]);

  const toggleSettingsView = useCallback(() => {
    setActiveView((current) =>
      current.kind === "settings"
        ? previousNonSettingsViewRef.current
        : { kind: "settings" },
    );
  }, []);

  return {
    activeView,
    setActiveView,
    toggleSettingsView,
  };
}
