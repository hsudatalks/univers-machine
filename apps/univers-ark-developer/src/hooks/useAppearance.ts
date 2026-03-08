import { useEffect, useRef, useState } from "react";
import { loadAppSettings, saveAppSettings } from "../lib/tauri";
import type { AppSettings, ThemeMode } from "../types";

type ResolvedTheme = "light" | "dark";

const DEFAULT_SETTINGS: AppSettings = {
  themeMode: "system",
  dashboardRefreshSeconds: 30,
};

function resolveSystemTheme(): ResolvedTheme {
  if (
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches
  ) {
    return "dark";
  }

  return "light";
}

export function useAppearance() {
  const [appSettings, setAppSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [resolvedTheme, setResolvedTheme] = useState<ResolvedTheme>(resolveSystemTheme);
  const didLoadSettingsRef = useRef(false);

  useEffect(() => {
    let cancelled = false;

    void loadAppSettings().then((settings) => {
      if (cancelled) {
        return;
      }

      setAppSettings(settings);
      didLoadSettingsRef.current = true;
    });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

    const applyTheme = () => {
      const nextResolvedTheme =
        appSettings.themeMode === "system"
          ? mediaQuery.matches
            ? "dark"
            : "light"
          : appSettings.themeMode;

      setResolvedTheme(nextResolvedTheme);
      document.documentElement.dataset.theme = nextResolvedTheme;
      document.documentElement.style.colorScheme = nextResolvedTheme;
    };

    applyTheme();

    const handleChange = () => {
      if (appSettings.themeMode === "system") {
        applyTheme();
      }
    };

    mediaQuery.addEventListener("change", handleChange);

    return () => {
      mediaQuery.removeEventListener("change", handleChange);
    };
  }, [appSettings.themeMode]);

  const updateSettings = (updater: (current: AppSettings) => AppSettings) => {
    setAppSettings((current) => {
      const next = updater(current);

      if (didLoadSettingsRef.current) {
        void saveAppSettings(next);
      }

      return next;
    });
  };

  const updateThemeMode = (themeMode: ThemeMode) => {
    updateSettings((current) => ({ ...current, themeMode }));
  };

  const updateDashboardRefreshSeconds = (dashboardRefreshSeconds: number) => {
    updateSettings((current) => ({ ...current, dashboardRefreshSeconds }));
  };

  return {
    appSettings,
    resolvedTheme,
    updateDashboardRefreshSeconds,
    updateThemeMode,
  };
}
