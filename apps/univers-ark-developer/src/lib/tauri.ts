import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppSettings,
  AppBootstrap,
  ConnectivityStatusBatch,
  ConnectivityStatusEvent,
  ContainerDashboard,
  ContainerDashboardUpdate,
  DeveloperService,
  DeveloperSurface,
  DeveloperTarget,
  GithubMergeMethod,
  GithubPullRequestDetail,
  GithubProjectState,
  MachineImportCandidate,
  ManagedContainer,
  ManagedMachine,
  RemoteDirectoryListing,
  RemoteFilePreview,
  ServiceStatus,
  TerminalExitEvent,
  TerminalOutputEvent,
  TerminalSnapshot,
  TunnelStatusBatch,
  TunnelStatus,
} from "../types";
import { browserSurfaceById } from "./target-services";

const SURFACE_PORT_START = import.meta.env.DEV ? 43000 : 45000;
const SURFACE_PORT_END = import.meta.env.DEV ? 43999 : 45999;
const SURFACE_HOST = "127.0.0.1";
const SIDEBAR_TOGGLE_REQUESTED_EVENT = "toggle-sidebar-requested";
const DASHBOARD_UPDATED_EVENT = "container-dashboard-updated";
const SERVICE_STATUS_EVENT = "service-status";
const PREVIOUS_CONTAINER_REQUESTED_EVENT = "previous-container-requested";
const NEXT_CONTAINER_REQUESTED_EVENT = "next-container-requested";
const PARENT_VIEW_REQUESTED_EVENT = "parent-view-requested";

const fallbackBootstrapSeed: AppBootstrap = {
  appName: "Univers Ark Developer",
  configPath: "developer-targets.json",
  selectedTargetId: "local::host",
  machines: [
    {
      id: "local",
      hostTargetId: "local::host",
      label: "Local",
      transport: "local",
      host: "localhost",
      description: "Local machine.",
      state: "ready",
      message: "Local host workspace is ready.",
      containers: [],
    },
  ],
  targets: [
    {
      id: "local::host",
      machineId: "local",
      containerId: "host",
      transport: "local",
      containerKind: "host",
      label: "Host",
      host: "localhost",
      description:
        "Local host workspace with direct development and preview surfaces on ports 3432 and 4173.",
      terminalCommand: "exec /bin/zsh -l",
      notes: [],
      workspace: {
        profile: "ark-workbench",
        defaultTool: "dashboard",
        projectPath: "~/repos/hvac-workbench",
        filesRoot: "~/repos/hvac-workbench",
        primaryWebServiceId: "development",
        tmuxCommandServiceId: "tmux-developer",
      },
      services: [
        {
          id: "development",
          label: "Development",
          kind: "web",
          description: "Primary Vite development surface.",
          web: {
            id: "development",
            label: "Development",
            serviceType: "vite",
            backgroundPrerender: true,
            tunnelCommand: "",
            localUrl: "http://127.0.0.1:3432/",
            remoteUrl: "http://127.0.0.1:3432/",
          },
        },
        {
          id: "preview",
          label: "Preview",
          kind: "web",
          description: "Preview surface.",
          web: {
            id: "preview",
            label: "Preview",
            serviceType: "http",
            backgroundPrerender: false,
            tunnelCommand: "",
            localUrl: "http://127.0.0.1:4173/",
            remoteUrl: "http://127.0.0.1:4173/",
          },
        },
        {
          id: "tmux-developer",
          label: "Developer Tmux",
          kind: "command",
          description: "Restart the developer tmux server.",
          command: {
            restart:
              "cd ~/repos/univers-container && ./.claude/skills/container-manage/bin/cm dev restart developer",
          },
        },
      ],
      surfaces: [
        {
          id: "development",
          label: "Development",
          serviceType: "vite",
          backgroundPrerender: true,
          tunnelCommand: "",
          localUrl: "http://127.0.0.1:3432/",
          remoteUrl: "http://127.0.0.1:3432/",
        },
        {
          id: "preview",
          label: "Preview",
          serviceType: "http",
          backgroundPrerender: false,
          tunnelCommand: "",
          localUrl: "http://127.0.0.1:4173/",
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

const fallbackAppSettings: AppSettings = {
  themeMode: "system",
  dashboardRefreshSeconds: 30,
};

type RawManagedContainer = Omit<ManagedContainer, "machineId" | "machineLabel"> & {
  machineId?: string;
  machineLabel?: string;
  serverId?: string;
  serverLabel?: string;
};

type RawManagedMachine = Omit<ManagedMachine, "containers"> & {
  containers: RawManagedContainer[];
};

type RawAppBootstrap = Omit<AppBootstrap, "machines"> & {
  machines?: RawManagedMachine[];
  servers?: RawManagedMachine[];
};

function normalizeManagedContainer(container: RawManagedContainer): ManagedContainer {
  return {
    ...container,
    machineId: container.machineId ?? container.serverId ?? "",
    machineLabel: container.machineLabel ?? container.serverLabel ?? "",
  };
}

function normalizeManagedMachine(machine: RawManagedMachine): ManagedMachine {
  return {
    ...machine,
    hostTargetId: machine.hostTargetId ?? `${machine.id}::host`,
    containers: machine.containers.map(normalizeManagedContainer),
  };
}

function normalizeBootstrap(bootstrap: RawAppBootstrap): AppBootstrap {
  const machines = (bootstrap.machines ?? bootstrap.servers ?? []).map(
    normalizeManagedMachine,
  );

  return {
    appName: bootstrap.appName,
    configPath: bootstrap.configPath,
    selectedTargetId: bootstrap.selectedTargetId,
    targets: bootstrap.targets,
    machines,
  };
}

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
  const resolvedSurfaces = target.surfaces.map((surface) =>
    resolveFallbackSurface(target.id, surface),
  );
  const resolvedServices =
    target.services.length > 0
      ? target.services.map((service) => ({
          ...service,
          web: service.web
            ? resolveFallbackSurface(target.id, service.web)
            : service.web,
        }))
      : resolvedSurfacesToServices(resolvedSurfaces);

  return {
    ...target,
      workspace:
      target.workspace ?? {
        profile: "",
        defaultTool: "dashboard",
        projectPath: "",
        filesRoot: "",
        primaryWebServiceId: "",
        tmuxCommandServiceId: "",
      },
    services: resolvedServices,
    surfaces: resolvedSurfaces,
  };
}

function resolvedSurfacesToServices(
  surfaces: DeveloperSurface[],
): DeveloperService[] {
  return surfaces.map((surface) => ({
    id: surface.id,
    label: surface.label,
    kind: "web",
    description: "",
    web: surface,
  }));
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
  return browserSurfaceById(fallbackTarget(targetId), surfaceId);
}

function fallbackTunnelStatus(
  targetId: string,
  serviceId: string,
): TunnelStatus {
  const surface = fallbackSurface(targetId, serviceId);

  if (!surface) {
    return {
      targetId,
      serviceId,
      surfaceId: serviceId,
      localUrl: null,
      state: "error",
      message: `Unknown web service: ${serviceId}`,
    };
  }

  if (!surface.tunnelCommand) {
    return {
      targetId,
      serviceId,
      surfaceId: serviceId,
      localUrl: surface.localUrl,
      state: "direct",
      message: `${surface.label} is using the local URL directly in browser mode.`,
    };
  }

  return {
    targetId,
    serviceId,
    surfaceId: serviceId,
    localUrl: surface.localUrl,
    state: "running",
    message: `Browser mode assumes the ${surface.label.toLowerCase()} tunnel is managed outside the app.`,
  };
}

export async function loadBootstrap(): Promise<AppBootstrap> {
  try {
    return normalizeBootstrap(await invoke<RawAppBootstrap>("load_bootstrap"));
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

  return normalizeBootstrap(await invoke<RawAppBootstrap>("refresh_bootstrap"));
}

export async function loadAppSettings(): Promise<AppSettings> {
  if (!isTauri()) {
    return fallbackAppSettings;
  }

  return invoke<AppSettings>("load_app_settings");
}

export async function saveAppSettings(settings: AppSettings): Promise<AppSettings> {
  if (!isTauri()) {
    return settings;
  }

  return invoke<AppSettings>("save_app_settings", { settings });
}

export async function updateRuntimeActivity(activity: {
  visible: boolean;
  focused: boolean;
  online: boolean;
  activeMachineId?: string | null;
  activeTargetId?: string | null;
}): Promise<void> {
  if (!isTauri()) {
    return;
  }

  await invoke("update_runtime_activity", { activity });
}

export async function loadMachineInventory(): Promise<ManagedMachine[]> {
  if (!isTauri()) {
    return fallbackBootstrap.machines;
  }

  return (await invoke<RawManagedMachine[]>("load_machine_inventory")).map(
    normalizeManagedMachine,
  );
}

export async function refreshMachineInventory(): Promise<ManagedMachine[]> {
  if (!isTauri()) {
    return fallbackBootstrap.machines;
  }

  return (await invoke<RawManagedMachine[]>("refresh_machine_inventory")).map(
    normalizeManagedMachine,
  );
}

export async function scanMachineInventory(machineId: string): Promise<ManagedMachine> {
  if (!isTauri()) {
    const machine = fallbackBootstrap.machines.find((item) => item.id === machineId);
    if (!machine) {
      throw new Error(`Unknown machine ${machineId}`);
    }
    return machine;
  }

  return normalizeManagedMachine(
    await invoke<RawManagedMachine>("scan_machine_inventory", { machineId }),
  );
}

export async function scanSshConfigMachineCandidates(): Promise<MachineImportCandidate[]> {
  if (!isTauri()) {
    throw new Error("SSH config scanning is only available in the desktop app.");
  }

  return invoke<MachineImportCandidate[]>("scan_ssh_config_machine_candidates");
}

export async function scanTailscaleMachineCandidates(): Promise<MachineImportCandidate[]> {
  if (!isTauri()) {
    throw new Error("Tailscale scanning is only available in the desktop app.");
  }

  return invoke<MachineImportCandidate[]>("scan_tailscale_machine_candidates");
}

export async function restartContainer(
  machineId: string,
  containerName: string,
): Promise<void> {
  if (!isTauri()) {
    return;
  }

  return invoke<void>("restart_container", {
    serverId: machineId,
    containerName,
  });
}

export async function loadServerInventory(): Promise<ManagedMachine[]> {
  return loadMachineInventory();
}

export async function refreshServerInventory(): Promise<ManagedMachine[]> {
  return refreshMachineInventory();
}

export async function scanServerInventory(serverId: string): Promise<ManagedMachine> {
  return scanMachineInventory(serverId);
}

export async function restartTmux(targetId: string): Promise<void> {
  if (!isTauri()) {
    return;
  }

  return invoke<void>("restart_tmux", { targetId });
}

export async function executeCommandService(
  targetId: string,
  serviceId: string,
  action: "restart",
): Promise<void> {
  if (!isTauri()) {
    return;
  }

  return invoke<void>("execute_command_service", {
    spec: { targetId, serviceId, action },
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

export async function loadTargetsConfig(): Promise<string> {
  if (!isTauri()) {
    return "{}";
  }

  return invoke<string>("load_targets_config");
}

export async function updateTargetsConfig(content: string): Promise<void> {
  if (!isTauri()) {
    return;
  }

  return invoke<void>("update_targets_config", { content });
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
  serviceId: string,
): Promise<TunnelStatus> {
  if (!isTauri()) {
    return fallbackTunnelStatus(targetId, serviceId);
  }

  return invoke<TunnelStatus>("ensure_tunnel", { targetId, serviceId });
}

export async function syncTunnelRegistrations(
  requests: Array<{ targetId: string; serviceId: string }>,
): Promise<TunnelStatus[]> {
  if (!isTauri()) {
    return requests.map(({ targetId, serviceId }) =>
      fallbackTunnelStatus(targetId, serviceId),
    );
  }

  return invoke<TunnelStatus[]>("sync_tunnel_registrations", { requests });
}

export async function restartTunnel(
  targetId: string,
  serviceId: string,
): Promise<TunnelStatus> {
  if (!isTauri()) {
    return fallbackTunnelStatus(targetId, serviceId);
  }

  return invoke<TunnelStatus>("restart_tunnel", { targetId, serviceId });
}

export async function restartAllTunnels(
  requests: Array<{ targetId: string; serviceId: string }>,
): Promise<TunnelStatus[]> {
  if (!isTauri()) {
    return requests.map(({ targetId, serviceId }) =>
      fallbackTunnelStatus(targetId, serviceId),
    );
  }

  return invoke<TunnelStatus[]>("restart_all_tunnels", { requests });
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

export async function listenTunnelStatusBatch(
  handler: (payload: TunnelStatusBatch) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<TunnelStatusBatch>("tunnel-status-batch", (event) => {
    handler(event.payload);
  });
}

export async function listenConnectivityStatus(
  handler: (payload: ConnectivityStatusEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<ConnectivityStatusEvent>("connectivity-status", (event) => {
    handler(event.payload);
  });
}

export async function listenConnectivityStatusBatch(
  handler: (payload: ConnectivityStatusBatch) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<ConnectivityStatusBatch>("connectivity-status-batch", (event) => {
    handler(event.payload);
  });
}

export async function listenServiceStatus(
  handler: (payload: ServiceStatus) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<ServiceStatus>(SERVICE_STATUS_EVENT, (event) => {
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

export async function listenPreviousContainerRequested(
  handler: () => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen(PREVIOUS_CONTAINER_REQUESTED_EVENT, () => {
    handler();
  });
}

export async function listenNextContainerRequested(
  handler: () => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen(NEXT_CONTAINER_REQUESTED_EVENT, () => {
    handler();
  });
}

export async function listenParentViewRequested(
  handler: () => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen(PARENT_VIEW_REQUESTED_EVENT, () => {
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

export async function loadContainerDashboard(
  targetId: string,
): Promise<ContainerDashboard> {
  if (!isTauri()) {
    return {
      targetId,
      project: {
        projectPath:
          fallbackTarget(targetId).workspace?.projectPath ||
          fallbackTarget(targetId).workspace?.filesRoot ||
          "~/repos",
        repoFound: false,
        branch: null,
        isDirty: false,
        changedFiles: 0,
        headSummary: null,
      },
      runtime: {
        hostname: "localhost",
        uptimeSeconds: 0,
        processCount: 0,
        loadAverage1m: 0,
        loadAverage5m: 0,
        loadAverage15m: 0,
        memoryTotalBytes: 0,
        memoryUsedBytes: 0,
        diskTotalBytes: 0,
        diskUsedBytes: 0,
      },
      services: [],
      agent: {
        activeAgent: "unknown",
        source: "none",
        lastActivity: null,
        latestReport: null,
        latestReportUpdatedAt: null,
      },
      tmux: {
        installed: false,
        serverRunning: false,
        sessionCount: 0,
        attachedCount: 0,
        activeSession: null,
        activeCommand: null,
        sessions: [],
      },
    };
  }

  return invoke<ContainerDashboard>("load_container_dashboard", { targetId });
}

export async function startDashboardMonitor(
  targetId: string,
  refreshSeconds: number,
): Promise<void> {
  if (!isTauri()) {
    return;
  }

  await invoke("start_dashboard_monitor", { refreshSeconds, targetId });
}

export async function stopDashboardMonitor(targetId: string): Promise<void> {
  if (!isTauri()) {
    return;
  }

  await invoke("stop_dashboard_monitor", { targetId });
}

export async function refreshContainerDashboard(targetId: string): Promise<void> {
  if (!isTauri()) {
    return;
  }

  await invoke("refresh_container_dashboard", { targetId });
}

export async function listenContainerDashboardUpdates(
  handler: (payload: ContainerDashboardUpdate) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<ContainerDashboardUpdate>(DASHBOARD_UPDATED_EVENT, (event) => {
    handler(event.payload);
  });
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
