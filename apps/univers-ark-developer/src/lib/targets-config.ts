import type {
  BrowserServiceType,
  CommandService,
  ContainerWorkspace,
  DeveloperService,
  DeveloperSurface,
  EndpointProbeType,
} from "../types";

export type ContainerManagerType = "lxd" | "docker" | "orbstack";
export type ContainerDiscoveryMode = "auto" | "manual";

export interface EditableEndpointService {
  probeType: EndpointProbeType;
  host: string;
  port: number;
  path: string;
  url: string;
}

export type EditableCommandService = CommandService;

export interface EditableDeveloperService extends Omit<DeveloperService, "web" | "endpoint" | "command"> {
  web?: DeveloperSurface | null;
  endpoint?: EditableEndpointService | null;
  command?: EditableCommandService | null;
}

export interface ContainerProfileConfig {
  workspace: ContainerWorkspace;
  services: EditableDeveloperService[];
  surfaces?: DeveloperSurface[];
}

export interface ManualContainerConfig {
  name: string;
  label: string;
  description: string;
  ipv4: string;
  status: string;
  workspace: ContainerWorkspace;
  services: EditableDeveloperService[];
  surfaces: DeveloperSurface[];
}

export interface RemoteServerConfig {
  id: string;
  label: string;
  host: string;
  description: string;
  managerType: ContainerManagerType;
  discoveryMode: ContainerDiscoveryMode;
  discoveryCommand: string;
  sshUser: string;
  sshOptions: string;
  containerNameSuffix: string;
  includeStopped: boolean;
  targetLabelTemplate: string;
  targetHostTemplate: string;
  targetDescriptionTemplate: string;
  terminalCommandTemplate: string;
  notes: string[];
  workspace: ContainerWorkspace;
  services: EditableDeveloperService[];
  surfaces: DeveloperSurface[];
  manualContainers: ManualContainerConfig[];
}

export interface TargetsConfigDocument {
  selectedTargetId?: string | null;
  profiles: Record<string, ContainerProfileConfig>;
  targets: Array<Record<string, unknown>>;
  remoteServers: RemoteServerConfig[];
}

function normalizeWorkspace(
  workspace: Partial<ContainerWorkspace> | undefined,
): ContainerWorkspace {
  return {
    ...createEmptyWorkspace(workspace?.profile ?? ""),
    ...workspace,
  };
}

function normalizeProfile(
  profileId: string,
  profile: Partial<ContainerProfileConfig> | undefined,
): ContainerProfileConfig {
  return {
    ...createEmptyProfile(profileId),
    ...profile,
    workspace: normalizeWorkspace(profile?.workspace),
    services: profile?.services ?? [],
    surfaces: profile?.surfaces ?? [],
  };
}

function normalizeManualContainer(
  container: Partial<ManualContainerConfig> | undefined,
): ManualContainerConfig {
  return {
    ...createEmptyManualContainer(),
    ...container,
    workspace: normalizeWorkspace(container?.workspace),
    services: container?.services ?? [],
    surfaces: container?.surfaces ?? [],
  };
}

function normalizeServer(
  server: Partial<RemoteServerConfig> | undefined,
): RemoteServerConfig {
  return {
    ...createEmptyServer(server?.workspace?.profile ?? ""),
    ...server,
    workspace: normalizeWorkspace(server?.workspace),
    services: server?.services ?? [],
    surfaces: server?.surfaces ?? [],
    manualContainers: (server?.manualContainers ?? []).map(normalizeManualContainer),
  };
}

export function parseTargetsConfig(raw: string): TargetsConfigDocument {
  const parsed = JSON.parse(raw) as Partial<TargetsConfigDocument>;
  const profiles = Object.fromEntries(
    Object.entries(parsed.profiles ?? {}).map(([profileId, profile]) => [
      profileId,
      normalizeProfile(profileId, profile),
    ]),
  );

  return {
    selectedTargetId: parsed.selectedTargetId ?? null,
    profiles,
    targets: parsed.targets ?? [],
    remoteServers: (parsed.remoteServers ?? []).map(normalizeServer),
  };
}

export function stringifyTargetsConfig(config: TargetsConfigDocument): string {
  return JSON.stringify(config, null, 2);
}

export function createEmptyWorkspace(profile = ""): ContainerWorkspace {
  return {
    profile,
    defaultTool: "dashboard",
    projectPath: "",
    filesRoot: "",
    primaryWebServiceId: "",
    primaryBrowserServiceId: "",
    tmuxCommandServiceId: "",
  };
}

export function createEmptyProfile(profileId = ""): ContainerProfileConfig {
  return {
    workspace: createEmptyWorkspace(profileId),
    services: [],
    surfaces: [],
  };
}

export function createDefaultWebService(
  id: string,
  label: string,
  serviceType: BrowserServiceType = "http",
): EditableDeveloperService {
  return {
    id,
    label,
    kind: "web",
    description: "",
    web: {
      id,
      label,
      serviceType,
      tunnelCommand: "",
      localUrl: "",
      remoteUrl: "",
      viteHmrTunnelCommand: "",
    },
    endpoint: null,
    command: null,
  };
}

export function createEmptyManualContainer(): ManualContainerConfig {
  return {
    name: "",
    label: "",
    description: "",
    ipv4: "",
    status: "RUNNING",
    workspace: createEmptyWorkspace(),
    services: [],
    surfaces: [],
  };
}

export function createEmptyServer(profileId = ""): RemoteServerConfig {
  return {
    id: "",
    label: "",
    host: "",
    description: "",
    managerType: "lxd",
    discoveryMode: "auto",
    discoveryCommand: "",
    sshUser: "ubuntu",
    sshOptions: "-o StrictHostKeyChecking=accept-new",
    containerNameSuffix: "-dev",
    includeStopped: false,
    targetLabelTemplate: "",
    targetHostTemplate: "{serverHost}",
    targetDescriptionTemplate: "",
    terminalCommandTemplate: "",
    notes: [],
    workspace: createEmptyWorkspace(profileId),
    services: [],
    surfaces: [],
    manualContainers: [],
  };
}
