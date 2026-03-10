import type {
  CommandService,
  DeveloperService,
  DeveloperSurface,
  DeveloperTarget,
  EndpointService,
} from "../types";
import type { ContainerToolPanel } from "./view-types";

export type BrowserDeveloperService = DeveloperService & {
  kind: "browser";
  browser: DeveloperSurface;
};

export type EndpointDeveloperService = DeveloperService & {
  kind: "endpoint";
  endpoint: EndpointService;
};

export type CommandDeveloperService = DeveloperService & {
  kind: "command";
  command: CommandService;
};

export function browserServices(target: DeveloperTarget): BrowserDeveloperService[] {
  if (target.services.length > 0) {
    return target.services.filter(
      (service): service is BrowserDeveloperService =>
        service.kind === "browser" && Boolean(service.browser),
    );
  }

  return target.surfaces.map((surface) => ({
    id: surface.id,
    label: surface.label,
    kind: "browser" as const,
    description: "",
    browser: surface,
  }));
}

export function endpointServices(target: DeveloperTarget): EndpointDeveloperService[] {
  return target.services.filter(
    (service): service is EndpointDeveloperService =>
      service.kind === "endpoint" && Boolean(service.endpoint),
  );
}

export function commandServices(target: DeveloperTarget): CommandDeveloperService[] {
  return target.services.filter(
    (service): service is CommandDeveloperService =>
      service.kind === "command" && Boolean(service.command),
  );
}

export function browserServiceById(
  target: DeveloperTarget,
  serviceId: string,
): BrowserDeveloperService | undefined {
  return browserServices(target).find((service) => service.id === serviceId);
}

export function endpointServiceById(
  target: DeveloperTarget,
  serviceId: string,
): EndpointDeveloperService | undefined {
  return endpointServices(target).find((service) => service.id === serviceId);
}

export function commandServiceById(
  target: DeveloperTarget,
  serviceId: string,
): CommandDeveloperService | undefined {
  return commandServices(target).find((service) => service.id === serviceId);
}

export function browserSurfaceById(
  target: DeveloperTarget,
  serviceId: string,
): DeveloperSurface | undefined {
  return browserServiceById(target, serviceId)?.browser;
}

export function primaryBrowserService(
  target: DeveloperTarget,
): BrowserDeveloperService | undefined {
  const preferredId = target.workspace?.primaryBrowserServiceId?.trim();

  if (preferredId) {
    const preferred = browserServiceById(target, preferredId);

    if (preferred) {
      return preferred;
    }
  }

  return browserServiceById(target, "development") ?? browserServices(target)[0];
}

export function primaryBrowserSurface(
  target: DeveloperTarget,
): DeveloperSurface | undefined {
  return primaryBrowserService(target)?.browser;
}

export function tmuxCommandService(
  target: DeveloperTarget,
): CommandDeveloperService | undefined {
  const preferredId = target.workspace?.tmuxCommandServiceId?.trim();

  if (preferredId) {
    const preferred = commandServiceById(target, preferredId);

    if (preferred) {
      return preferred;
    }
  }

  return commandServiceById(target, "tmux-developer") ?? commandServices(target)[0];
}

export function resolveDefaultToolPanel(target: DeveloperTarget): ContainerToolPanel {
  const defaultTool = target.workspace?.defaultTool?.trim();

  if (defaultTool === "files" || defaultTool === "dashboard") {
    return defaultTool;
  }

  if (defaultTool === "browser") {
    const primary = primaryBrowserService(target);
    return primary ? (`browser:${primary.id}` as const) : "dashboard";
  }

  if (defaultTool?.startsWith("browser:")) {
    const serviceId = defaultTool.slice("browser:".length);
    return browserServiceById(target, serviceId)
      ? (`browser:${serviceId}` as const)
      : "dashboard";
  }

  if (defaultTool && browserServiceById(target, defaultTool)) {
    return `browser:${defaultTool}`;
  }

  return "dashboard";
}
