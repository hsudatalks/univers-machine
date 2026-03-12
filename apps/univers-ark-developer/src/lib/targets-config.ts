import type {
  BrowserServiceType,
  CommandService,
  ContainerWorkspace,
  DeveloperService,
  DeveloperSurface,
  EndpointProbeType,
} from "../types";

export type MachineTransport = "local" | "ssh";
export type ContainerManagerType = "none" | "lxd" | "docker" | "orbstack";
export type ContainerDiscoveryMode = "host-only" | "auto" | "manual";
export type MachineContainerKind = "host" | "managed";
export type MachineContainerSource =
  | "host"
  | "manual"
  | "orbstack"
  | "docker"
  | "lxd"
  | "custom"
  | "unknown";

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
  extends?: string;
  workspace: ContainerWorkspace;
  services: EditableDeveloperService[];
  surfaces?: DeveloperSurface[];
}

export interface SshJumpConfig {
  host: string;
  port: number;
  user: string;
  identityFiles: string[];
  sshCredentialId: string;
}

export interface MachineContainerConfig {
  id: string;
  name: string;
  kind: MachineContainerKind;
  enabled: boolean;
  source: MachineContainerSource;
  sshUser: string;
  sshUserCandidates: string[];
  label: string;
  description: string;
  ipv4: string;
  status: string;
  workspace: ContainerWorkspace;
  services: EditableDeveloperService[];
  surfaces: DeveloperSurface[];
}

export interface MachineConfig {
  id: string;
  label: string;
  transport: MachineTransport;
  host: string;
  port: number;
  description: string;
  managerType: ContainerManagerType;
  discoveryMode: ContainerDiscoveryMode;
  discoveryCommand: string;
  sshUser: string;
  containerSshUser: string;
  identityFiles: string[];
  sshCredentialId: string;
  jumpChain: SshJumpConfig[];
  knownHostsPath: string;
  strictHostKeyChecking: boolean;
  containerNameSuffix: string;
  includeStopped: boolean;
  targetLabelTemplate: string;
  targetHostTemplate: string;
  targetDescriptionTemplate: string;
  hostTerminalStartupCommand: string;
  terminalCommandTemplate: string;
  notes: string[];
  workspace: ContainerWorkspace;
  services: EditableDeveloperService[];
  surfaces: DeveloperSurface[];
  containers: MachineContainerConfig[];
}

export interface TargetsConfigDocument {
  selectedTargetId?: string | null;
  defaultProfile?: string | null;
  profiles: Record<string, ContainerProfileConfig>;
  machines: MachineConfig[];
}

function normalizeWorkspace(
  workspace: Partial<ContainerWorkspace> | undefined,
): ContainerWorkspace {
  return {
    profile: workspace?.profile ?? "",
    defaultTool: workspace?.defaultTool ?? "dashboard",
    projectPath: workspace?.projectPath ?? "",
    filesRoot: workspace?.filesRoot ?? "",
    primaryWebServiceId:
      workspace?.primaryWebServiceId ?? workspace?.primaryBrowserServiceId ?? "",
    tmuxCommandServiceId: workspace?.tmuxCommandServiceId ?? "",
  };
}

function normalizeProfile(
  profileId: string,
  profile: Partial<ContainerProfileConfig> | undefined,
): ContainerProfileConfig {
  return {
    ...createEmptyProfile(profileId),
    ...profile,
    extends: profile?.extends ?? "",
    workspace: normalizeWorkspace(profile?.workspace),
    services: profile?.services ?? [],
    surfaces: profile?.surfaces ?? [],
  };
}

function normalizeMachineContainer(
  container: Partial<MachineContainerConfig> | undefined,
): MachineContainerConfig {
  const sshUser = container?.sshUser ?? "";
  const sshUserCandidates = Array.from(
    new Set(
      [sshUser, ...(container?.sshUserCandidates ?? [])].filter(
        (candidate): candidate is string => Boolean(candidate?.trim()),
      ),
    ),
  );

  return {
    ...createEmptyMachineContainer(),
    ...container,
    source:
      container?.source ??
      (container?.kind === "host"
        ? "host"
        : container?.ipv4?.trim()
          ? "unknown"
          : "manual"),
    sshUser,
    sshUserCandidates,
    workspace: normalizeWorkspace(container?.workspace),
    services: container?.services ?? [],
    surfaces: container?.surfaces ?? [],
  };
}

function normalizeMachine(
  machine: Partial<MachineConfig> | undefined,
): MachineConfig {
  return {
    ...createEmptyMachine(machine?.workspace?.profile ?? ""),
    ...machine,
    containerSshUser:
      machine?.containerSshUser ?? machine?.sshUser ?? "ubuntu",
    workspace: normalizeWorkspace(machine?.workspace),
    services: machine?.services ?? [],
    surfaces: machine?.surfaces ?? [],
    identityFiles: machine?.identityFiles ?? [],
    jumpChain: (machine?.jumpChain ?? []).map((jump) => ({
      host: jump.host ?? "",
      port: jump.port ?? 22,
      user: jump.user ?? "",
      identityFiles: jump.identityFiles ?? [],
      sshCredentialId: jump.sshCredentialId ?? "",
    })),
    containers: (machine?.containers ?? [])
      .filter((container) => container?.kind !== "host")
      .map(normalizeMachineContainer),
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
    defaultProfile: parsed.defaultProfile ?? null,
    profiles,
    machines: (parsed.machines ?? []).map(normalizeMachine),
  };
}

export function stringifyTargetsConfig(config: TargetsConfigDocument): string {
  return JSON.stringify(
    {
      selectedTargetId: config.selectedTargetId ?? null,
      defaultProfile: config.defaultProfile ?? null,
      profiles: config.profiles,
      machines: config.machines,
    },
    null,
    2,
  );
}

export function createEmptyWorkspace(profile = ""): ContainerWorkspace {
  return {
    profile,
    defaultTool: "dashboard",
    projectPath: "",
    filesRoot: "",
    primaryWebServiceId: "",
    tmuxCommandServiceId: "",
  };
}

export function createEmptyProfile(profileId = ""): ContainerProfileConfig {
  return {
    extends: "",
    workspace: createEmptyWorkspace(profileId),
    services: [],
    surfaces: [],
  };
}

export function createEmptySshJump(): SshJumpConfig {
  return {
    host: "",
    port: 22,
    user: "",
    identityFiles: [],
    sshCredentialId: "",
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
      backgroundPrerender: serviceType === "vite",
      tunnelCommand: "",
      localUrl: "",
      remoteUrl: "",
      viteHmrTunnelCommand: "",
    },
    endpoint: null,
    command: null,
  };
}

export function createDefaultEndpointService(
  id: string,
  label: string,
  probeType: EndpointProbeType = "http",
): EditableDeveloperService {
  return {
    id,
    label,
    kind: "endpoint",
    description: "",
    web: null,
    endpoint: {
      probeType,
      host: "127.0.0.1",
      port: 0,
      path: "",
      url: "",
    },
    command: null,
  };
}

export function createDefaultCommandService(
  id: string,
  label: string,
): EditableDeveloperService {
  return {
    id,
    label,
    kind: "command",
    description: "",
    web: null,
    endpoint: null,
    command: {
      restart: "",
    },
  };
}

export function createEmptyMachineContainer(): MachineContainerConfig {
  return {
    id: "",
    name: "",
    kind: "managed",
    enabled: true,
    source: "manual",
    sshUser: "",
    sshUserCandidates: [],
    label: "",
    description: "",
    ipv4: "",
    status: "RUNNING",
    workspace: createEmptyWorkspace(),
    services: [],
    surfaces: [],
  };
}

export function createEmptyMachine(profileId = ""): MachineConfig {
  return {
    id: "",
    label: "",
    transport: "ssh",
    host: "",
    port: 22,
    description: "",
    managerType: "none",
    discoveryMode: "auto",
    discoveryCommand: "",
    sshUser: "ubuntu",
    containerSshUser: "ubuntu",
    identityFiles: [],
    sshCredentialId: "",
    jumpChain: [],
    knownHostsPath: "~/.univers/known_hosts",
    strictHostKeyChecking: true,
    containerNameSuffix: "-dev",
    includeStopped: false,
    targetLabelTemplate: "",
    targetHostTemplate: "{machineHost}",
    targetDescriptionTemplate: "",
    hostTerminalStartupCommand: "",
    terminalCommandTemplate: "",
    notes: [],
    workspace: createEmptyWorkspace(profileId),
    services: [],
    surfaces: [],
    containers: [],
  };
}
