import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppBootstrap,
  DeveloperSurface,
  DeveloperTarget,
  GithubMergeMethod,
  GithubPullRequestDetail,
  GithubProjectState,
  ManagedServer,
  RemoteDirectoryListing,
  RemoteFilePreview,
  TerminalExitEvent,
  TerminalOutputEvent,
  TerminalSnapshot,
  TunnelStatus,
} from "../types";

const SURFACE_PORT_START = 43000;
const SURFACE_PORT_END = 43999;
const SURFACE_HOST = "127.0.0.1";
const SIDEBAR_TOGGLE_REQUESTED_EVENT = "toggle-sidebar-requested";

const fallbackBootstrapSeed: AppBootstrap = {
  appName: "Univers Ark Developer",
  configPath: "developer-targets.json",
  selectedTargetId: "automation-dev",
  servers: [],
  targets: [
    {
      id: "local",
      label: "Local",
      host: "localhost",
      description:
        "Local shell with direct development and preview surfaces on ports 3432 and 4173.",
      terminalCommand: "exec /bin/zsh -l",
      notes: [],
      surfaces: [
        {
          id: "development",
          label: "Development",
          tunnelCommand: "",
          localUrl: "http://127.0.0.1:3432/",
          remoteUrl: "http://127.0.0.1:3432/",
        },
        {
          id: "preview",
          label: "Preview",
          tunnelCommand: "",
          localUrl: "http://127.0.0.1:4173/",
          remoteUrl: "http://127.0.0.1:4173/",
        },
      ],
    },
    {
      id: "automation-dev",
      label: "Automation",
      host: "automation-dev",
      description:
        "LXD container profile for the automation workspace with separate development and preview browser surfaces.",
      terminalCommand: "ssh automation-dev",
      notes: [
        "Development maps the container's 3432 port for the live app surface.",
        "Preview maps the container's 4173 port for the production-like preview build.",
      ],
      surfaces: [
        {
          id: "development",
          label: "Development",
          tunnelCommand:
            "ssh -NT -L {localPort}:127.0.0.1:3432 automation-dev",
          viteHmrTunnelCommand:
            "ssh -NT -L {localPort}:127.0.0.1:3433 automation-dev",
          localUrl: "http://127.0.0.1:{localPort}/",
          remoteUrl: "http://127.0.0.1:3432/",
        },
        {
          id: "preview",
          label: "Preview",
          tunnelCommand:
            "ssh -NT -L {localPort}:127.0.0.1:4173 automation-dev",
          localUrl: "http://127.0.0.1:{localPort}/",
          remoteUrl: "http://127.0.0.1:4173/",
        },
      ],
    },
  ],
};

const fallbackBootstrap: AppBootstrap = {
  ...fallbackBootstrapSeed,
  targets: fallbackBootstrapSeed.targets.map(resolveFallbackTarget),
};

function surfaceKey(targetId: string, surfaceId: string): string {
  return `${targetId}::${surfaceId}`;
}

function stableSurfacePort(targetId: string, surfaceId: string): number {
  let hash = 2166136261;
  const key = surfaceKey(targetId, surfaceId);

  for (const char of key) {
    hash ^= char.charCodeAt(0);
    hash = Math.imul(hash, 16777619);
  }

  const span = SURFACE_PORT_END - SURFACE_PORT_START + 1;
  return SURFACE_PORT_START + (Math.abs(hash) % span);
}

function replaceKnownTunnelPlaceholders(
  tunnelCommand: string,
  remoteUrl: string,
  localPort: number,
): string {
  let nextCommand = tunnelCommand.replaceAll("{localPort}", String(localPort));

  try {
    const parsed = new URL(remoteUrl);
    const remotePort =
      parsed.port ||
      (parsed.protocol === "https:" ? "443" : parsed.protocol === "http:" ? "80" : "");

    nextCommand = nextCommand.replaceAll("{remoteHost}", parsed.hostname);
    nextCommand = nextCommand.replaceAll("{previewRemoteHost}", parsed.hostname);

    if (remotePort) {
      nextCommand = nextCommand.replaceAll("{remotePort}", remotePort);
      nextCommand = nextCommand.replaceAll("{previewRemotePort}", remotePort);
    }
  } catch {
    return nextCommand;
  }

  return nextCommand;
}

function rewriteForwardSpec(forwardSpec: string, localPort: number): string {
  const separatorIndex = forwardSpec.indexOf(":");

  if (separatorIndex === -1) {
    return forwardSpec;
  }

  return `${localPort}${forwardSpec.slice(separatorIndex)}`;
}

function rewriteTunnelForwardPort(
  tunnelCommand: string,
  localPort: number,
): string {
  const tokens = tunnelCommand.trim().split(/\s+/);

  for (let index = 0; index < tokens.length; index += 1) {
    if (tokens[index] === "-L" && tokens[index + 1]) {
      tokens[index + 1] = rewriteForwardSpec(tokens[index + 1], localPort);
      return tokens.join(" ");
    }

    if (tokens[index].startsWith("-L")) {
      tokens[index] = `-L${rewriteForwardSpec(tokens[index].slice(2), localPort)}`;
      return tokens.join(" ");
    }
  }

  return tunnelCommand;
}

function resolveSurfaceLocalUrl(
  localUrl: string,
  remoteUrl: string,
  localPort: number,
): string {
  const template = (localUrl || remoteUrl).replaceAll(
    "{localPort}",
    String(localPort),
  );

  try {
    const parsed = new URL(template);
    parsed.hostname = SURFACE_HOST;
    parsed.port = String(localPort);
    return parsed.toString();
  } catch {
    try {
      const parsed = new URL(remoteUrl);
      parsed.hostname = SURFACE_HOST;
      parsed.port = String(localPort);
      return parsed.toString();
    } catch {
      return `http://${SURFACE_HOST}:${localPort}/`;
    }
  }
}

function resolveSurfaceTunnelCommand(
  surface: DeveloperSurface,
  localPort: number,
): string {
  const placeholderResolved = replaceKnownTunnelPlaceholders(
    surface.tunnelCommand,
    surface.remoteUrl,
    localPort,
  );

  if (placeholderResolved !== surface.tunnelCommand) {
    return placeholderResolved;
  }

  return rewriteTunnelForwardPort(placeholderResolved, localPort);
}

function resolveFallbackSurface(
  targetId: string,
  surface: DeveloperSurface,
): DeveloperSurface {
  if (!surface.tunnelCommand.trim()) {
    return surface;
  }

  const localPort = stableSurfacePort(targetId, surface.id);

  return {
    ...surface,
    tunnelCommand: resolveSurfaceTunnelCommand(surface, localPort),
    localUrl: resolveSurfaceLocalUrl(surface.localUrl, surface.remoteUrl, localPort),
  };
}

function resolveFallbackTarget(target: DeveloperTarget): DeveloperTarget {
  return {
    ...target,
    surfaces: target.surfaces.map((surface) =>
      resolveFallbackSurface(target.id, surface),
    ),
  };
}

function fallbackTarget(targetId: string): DeveloperTarget {
  return (
    fallbackBootstrap.targets.find((target) => target.id === targetId) ??
    fallbackBootstrap.targets[0]
  );
}

function fallbackSurface(
  targetId: string,
  surfaceId: string,
): DeveloperSurface | undefined {
  return fallbackTarget(targetId).surfaces.find((surface) => surface.id === surfaceId);
}

function fallbackTunnelStatus(
  targetId: string,
  surfaceId: string,
): TunnelStatus {
  const surface = fallbackSurface(targetId, surfaceId);

  if (!surface) {
    return {
      targetId,
      surfaceId,
      state: "error",
      message: `Unknown browser surface: ${surfaceId}`,
    };
  }

  if (!surface.tunnelCommand) {
    return {
      targetId,
      surfaceId,
      state: "direct",
      message: `${surface.label} is using the local URL directly in browser mode.`,
    };
  }

  return {
    targetId,
    surfaceId,
    state: "running",
    message: `Browser mode assumes the ${surface.label.toLowerCase()} tunnel is managed outside the app.`,
  };
}

export async function loadBootstrap(): Promise<AppBootstrap> {
  try {
    return await invoke<AppBootstrap>("load_bootstrap");
  } catch (error) {
    console.warn(
      "Falling back to bundled developer targets because the Tauri backend is not available yet.",
      error,
    );
    return fallbackBootstrap;
  }
}

export async function refreshBootstrap(): Promise<AppBootstrap> {
  if (!isTauri()) {
    return fallbackBootstrap;
  }

  return invoke<AppBootstrap>("refresh_bootstrap");
}

export async function loadServerInventory(): Promise<ManagedServer[]> {
  if (!isTauri()) {
    return fallbackBootstrap.servers;
  }

  return invoke<ManagedServer[]>("load_server_inventory");
}

export async function refreshServerInventory(): Promise<ManagedServer[]> {
  if (!isTauri()) {
    return fallbackBootstrap.servers;
  }

  return invoke<ManagedServer[]>("refresh_server_inventory");
}

export async function restartContainer(
  serverId: string,
  containerName: string,
): Promise<void> {
  if (!isTauri()) {
    return;
  }

  return invoke<void>("restart_container", {
    serverId,
    containerName,
  });
}

export async function clipboardWrite(text: string): Promise<void> {
  if (!isTauri()) {
    return;
  }

  return invoke<void>("clipboard_write", { text });
}

export async function clipboardRead(): Promise<string> {
  if (!isTauri()) {
    return "";
  }

  return invoke<string>("clipboard_read");
}

export async function attachTerminal(
  targetId: string,
): Promise<TerminalSnapshot> {
  if (!isTauri()) {
    const target = fallbackTarget(targetId);
    return {
      targetId: target.id,
      output: [
        `$ ${target.terminalCommand}`,
        "",
        "Tauri backend is not available in plain browser mode.",
      ].join("\r\n"),
    };
  }

  return invoke<TerminalSnapshot>("attach_terminal", { targetId });
}

export async function restartTerminal(
  targetId: string,
): Promise<TerminalSnapshot> {
  if (!isTauri()) {
    return attachTerminal(targetId);
  }

  return invoke<TerminalSnapshot>("restart_terminal", { targetId });
}

export async function writeTerminal(
  targetId: string,
  data: string,
): Promise<void> {
  if (!isTauri()) {
    return;
  }

  await invoke("write_terminal", { data, targetId });
}

export async function resizeTerminal(
  targetId: string,
  cols: number,
  rows: number,
): Promise<void> {
  if (!isTauri()) {
    return;
  }

  await invoke("resize_terminal", { cols, rows, targetId });
}

export async function listenTerminalOutput(
  handler: (payload: TerminalOutputEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<TerminalOutputEvent>("terminal-output", (event) => {
    handler(event.payload);
  });
}

export async function listenTerminalExit(
  handler: (payload: TerminalExitEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<TerminalExitEvent>("terminal-exit", (event) => {
    handler(event.payload);
  });
}

export async function ensureTunnel(
  targetId: string,
  surfaceId: string,
): Promise<TunnelStatus> {
  if (!isTauri()) {
    return fallbackTunnelStatus(targetId, surfaceId);
  }

  return invoke<TunnelStatus>("ensure_tunnel", { targetId, surfaceId });
}

export async function restartTunnel(
  targetId: string,
  surfaceId: string,
): Promise<TunnelStatus> {
  if (!isTauri()) {
    return fallbackTunnelStatus(targetId, surfaceId);
  }

  return invoke<TunnelStatus>("restart_tunnel", { targetId, surfaceId });
}

export async function listenTunnelStatus(
  handler: (payload: TunnelStatus) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<TunnelStatus>("tunnel-status", (event) => {
    handler(event.payload);
  });
}

export async function listenSidebarToggleRequested(
  handler: () => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen(SIDEBAR_TOGGLE_REQUESTED_EVENT, () => {
    handler();
  });
}

export async function listRemoteDirectory(
  targetId: string,
  path?: string | null,
): Promise<RemoteDirectoryListing> {
  if (!isTauri()) {
    return {
      targetId,
      path: "~",
      parentPath: null,
      entries: [],
    };
  }

  return invoke<RemoteDirectoryListing>("list_remote_directory", {
    path: path ?? null,
    targetId,
  });
}

export async function readRemoteFilePreview(
  targetId: string,
  path: string,
): Promise<RemoteFilePreview> {
  if (!isTauri()) {
    return {
      targetId,
      path,
      content: "Remote file previews require the Tauri backend.",
      isBinary: false,
      truncated: false,
    };
  }

  return invoke<RemoteFilePreview>("read_remote_file_preview", { path, targetId });
}

export async function loadGithubProjectState(): Promise<GithubProjectState> {
  if (!isTauri()) {
    throw new Error("GitHub project state requires the Tauri backend.");
  }

  return invoke<GithubProjectState>("load_github_project_state");
}

export async function openExternalLink(url: string): Promise<void> {
  if (!isTauri()) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }

  await invoke("open_external_link", { url });
}

export async function loadGithubPullRequestDetail(
  number: number,
): Promise<GithubPullRequestDetail> {
  if (!isTauri()) {
    throw new Error("GitHub pull request details require the Tauri backend.");
  }

  return invoke<GithubPullRequestDetail>("load_github_pull_request_detail", { number });
}

export async function mergeGithubPullRequest(
  number: number,
  method: GithubMergeMethod,
): Promise<void> {
  if (!isTauri()) {
    throw new Error("GitHub pull request merge requires the Tauri backend.");
  }

  await invoke("merge_github_pull_request", { method, number });
}
