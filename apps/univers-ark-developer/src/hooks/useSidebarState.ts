import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { listenSidebarToggleRequested } from "../lib/tauri";

const IS_MAC = navigator.platform.toUpperCase().includes("MAC");
const SIDEBAR_VISIBILITY_STORAGE_KEY =
  "univers-ark-developer:sidebar-hidden";

function isPlatformModifier(event: KeyboardEvent): boolean {
  return IS_MAC ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

export function useSidebarState() {
  const [isSidebarHidden, setIsSidebarHidden] = useState(() => {
    if (typeof window === "undefined") {
      return false;
    }

    return (
      window.localStorage.getItem(SIDEBAR_VISIBILITY_STORAGE_KEY) === "true"
    );
  });

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

  return {
    isSidebarHidden,
    setIsSidebarHidden,
  };
}
