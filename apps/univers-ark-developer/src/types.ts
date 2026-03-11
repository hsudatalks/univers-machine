export type BrowserServiceType = "http" | "vite";
export type DeveloperServiceKind = "web" | "endpoint" | "command";
export type EndpointProbeType = "http" | "tcp";
export type MachineTransport = "local" | "ssh";
export type ManagedContainerKind = "host" | "managed";

export interface DeveloperSurface {
  id: string;
  label: string;
  serviceType: BrowserServiceType;
  backgroundPrerender?: boolean;
  tunnelCommand: string;
  localUrl: string;
  remoteUrl: string;
  viteHmrTunnelCommand?: string;
}

export interface DeveloperService {
  id: string;
  label: string;
  kind: DeveloperServiceKind;
  description: string;
  web?: DeveloperSurface | null;
  endpoint?: EndpointService | null;
  command?: CommandService | null;
}

export interface EndpointService {
  probeType: EndpointProbeType;
  host: string;
  port: number;
  path: string;
  url: string;
}

export interface CommandService {
  restart: string;
}

export interface ContainerWorkspace {
  profile: string;
  defaultTool: string;
  projectPath: string;
  filesRoot: string;
  primaryWebServiceId?: string;
  primaryBrowserServiceId?: string;
  tmuxCommandServiceId: string;
}

export interface DeveloperTarget {
  id: string;
  machineId: string;
  containerId: string;
  transport: MachineTransport;
  containerKind: ManagedContainerKind;
  label: string;
  host: string;
  description: string;
  terminalCommand: string;
  terminalStartupCommand?: string;
  notes: string[];
  workspace: ContainerWorkspace;
  services: DeveloperService[];
  surfaces: DeveloperSurface[];
}

export interface ManagedContainer {
  machineId: string;
  machineLabel: string;
  containerId: string;
  kind: ManagedContainerKind;
  transport: MachineTransport;
  targetId: string;
  name: string;
  label: string;
  status: string;
  ipv4: string;
  sshUser: string;
  sshDestination: string;
  sshCommand: string;
  sshState: string;
  sshMessage: string;
  sshReachable: boolean;
}

export interface ManagedMachine {
  id: string;
  hostTargetId: string;
  label: string;
  transport: MachineTransport;
  host: string;
  description: string;
  state: string;
  message: string;
  containers: ManagedContainer[];
}

export type ManagedServer = ManagedMachine;

export interface ImportedMachineJump {
  host: string;
  port: number;
  user: string;
  identityFiles: string[];
}

export interface MachineImportCandidate {
  importId: string;
  machineId: string;
  label: string;
  host: string;
  port: number;
  sshUser: string;
  identityFiles: string[];
  jumpChain: ImportedMachineJump[];
  description: string;
  detail: string;
}

export interface RemoteFileEntry {
  name: string;
  path: string;
  kind: string;
  size: number;
  isHidden: boolean;
}

export interface RemoteDirectoryListing {
  targetId: string;
  path: string;
  parentPath: string | null;
  entries: RemoteFileEntry[];
}

export interface RemoteFilePreview {
  targetId: string;
  path: string;
  content: string;
  isBinary: boolean;
  truncated: boolean;
}

export interface ContainerProjectInfo {
  projectPath: string;
  repoFound: boolean;
  branch: string | null;
  isDirty: boolean;
  changedFiles: number;
  headSummary: string | null;
}

export interface ContainerRuntimeInfo {
  hostname: string;
  uptimeSeconds: number;
  processCount: number;
  loadAverage1m: number;
  loadAverage5m: number;
  loadAverage15m: number;
  memoryTotalBytes: number;
  memoryUsedBytes: number;
  diskTotalBytes: number;
  diskUsedBytes: number;
}

export interface ContainerServiceInfo {
  id: string;
  label: string;
  status: string;
  detail: string;
  url: string | null;
}

export interface ContainerAgentInfo {
  activeAgent: string;
  source: string;
  lastActivity: string | null;
  latestReport: string | null;
  latestReportUpdatedAt: string | null;
}

export interface ContainerTmuxSessionInfo {
  server: string;
  name: string;
  windows: number;
  attached: boolean;
  activeCommand: string | null;
}

export interface ContainerTmuxInfo {
  installed: boolean;
  serverRunning: boolean;
  sessionCount: number;
  attachedCount: number;
  activeSession: string | null;
  activeCommand: string | null;
  sessions: ContainerTmuxSessionInfo[];
}

export interface ContainerDashboard {
  targetId: string;
  project: ContainerProjectInfo;
  runtime: ContainerRuntimeInfo;
  services: ContainerServiceInfo[];
  agent: ContainerAgentInfo;
  tmux: ContainerTmuxInfo;
}

export interface ContainerDashboardUpdate {
  targetId: string;
  dashboard: ContainerDashboard | null;
  error: string | null;
  refreshedAtMs: number;
  refreshSeconds: number;
}

export interface AppBootstrap {
  appName: string;
  configPath: string;
  selectedTargetId: string | null;
  targets: DeveloperTarget[];
  machines: ManagedMachine[];
}

export type ThemeMode = "system" | "light" | "dark";

export interface AppSettings {
  themeMode: ThemeMode;
  dashboardRefreshSeconds: number;
}

export interface TerminalSnapshot {
  targetId: string;
  output: string;
}

export interface TerminalOutputEvent {
  targetId: string;
  data: string;
}

export interface TerminalExitEvent {
  targetId: string;
  reason: string;
}

export interface TunnelStatus {
  targetId: string;
  serviceId: string;
  surfaceId: string;
  localUrl: string | null;
  state: string;
  message: string;
}

export interface ConnectivityStatusEvent {
  entity: "machine" | "container";
  machineId: string;
  targetId?: string | null;
  state: string;
  message: string;
  reachable: boolean;
}

export interface ServiceStatus {
  targetId: string;
  serviceId: string;
  kind: DeveloperServiceKind;
  state: string;
  message: string;
  localUrl: string | null;
}

export interface GithubPullRequestSummary {
  number: number;
  title: string;
  url: string;
  authorLogin: string;
  headRefName: string;
  isDraft: boolean;
  state: string;
  reviewDecision: string | null;
  updatedAt: string;
}

export interface GithubRepositoryStatus {
  nameWithOwner: string;
  description: string;
  url: string;
  defaultBranch: string;
  viewerLogin: string;
  localRepoPath: string | null;
  localBranch: string | null;
  localStatusSummary: string | null;
}

export interface GithubProjectState {
  repository: GithubRepositoryStatus;
  currentBranchPr: GithubPullRequestSummary | null;
  myOpenPrs: GithubPullRequestSummary[];
  openPrs: GithubPullRequestSummary[];
  closedPrs: GithubPullRequestSummary[];
  mergedPrs: GithubPullRequestSummary[];
}

export type GithubMergeMethod = "merge" | "squash" | "rebase";

export interface GithubPullRequestFile {
  path: string;
  additions: number;
  deletions: number;
}

export interface GithubPullRequestReview {
  authorLogin: string;
  state: string;
  body: string;
  submittedAt: string;
}

export interface GithubStatusCheck {
  kind: string;
  name: string;
  status: string;
  conclusion: string | null;
  workflowName: string | null;
  detailsUrl: string;
}

export interface GithubPullRequestDetail {
  number: number;
  title: string;
  body: string;
  url: string;
  authorLogin: string;
  headRefName: string;
  baseRefName: string;
  isDraft: boolean;
  state: string;
  reviewDecision: string | null;
  updatedAt: string;
  mergeStateStatus: string;
  mergeable: string;
  changedFiles: number;
  additions: number;
  deletions: number;
  files: GithubPullRequestFile[];
  latestReviews: GithubPullRequestReview[];
  statusChecks: GithubStatusCheck[];
}
