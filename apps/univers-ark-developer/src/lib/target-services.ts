import type {
  CommandService,
  DeveloperService,
  DeveloperSurface,
  DeveloperTarget,
  EndpointService,
} from "../types";
import type { ContainerToolPanel } from "./view-types";

export type WebDeveloperService = DeveloperService & {
  kind: "web";
  web: DeveloperSurface;
};

export type EndpointDeveloperService = DeveloperService & {
  kind: "endpoint";
  endpoint: EndpointService;
};

export type CommandDeveloperService = DeveloperService & {
  kind: "command";
  command: CommandService;
};

export function webServices(target: DeveloperTarget): WebDeveloperService[] {
  if (target.services.length > 0) {
    return target.services.filter(
      (service): service is WebDeveloperService =>
        service.kind === "web" && Boolean(service.web),
    );
  }

  return target.surfaces.map((surface) => ({
    id: surface.id,
    label: surface.label,
    kind: "web" as const,
    description: "",
    web: surface,
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

export function webServiceById(
  target: DeveloperTarget,
  serviceId: string,
): WebDeveloperService | undefined {
  return webServices(target).find((service) => service.id === serviceId);
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

export function webSurfaceById(
  target: DeveloperTarget,
  serviceId: string,
): DeveloperSurface | undefined {
  return webServiceById(target, serviceId)?.web;
}

export function primaryWebService(target: DeveloperTarget): WebDeveloperService | undefined {
  const preferredId =
    target.workspace?.primaryWebServiceId?.trim() ||
    target.workspace?.primaryBrowserServiceId?.trim();

  if (preferredId) {
    const preferred = webServiceById(target, preferredId);

    if (preferred) {
      return preferred;
    }
  }

  return webServices(target)[0];
}

export function primaryWebSurface(target: DeveloperTarget): DeveloperSurface | undefined {
  return primaryWebService(target)?.web;
}

export function backgroundPrerenderWebServices(
  target: DeveloperTarget,
): WebDeveloperService[] {
  return webServices(target).filter((service) => service.web.backgroundPrerender);
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

  if (
    defaultTool === "files" ||
    defaultTool === "dashboard" ||
    defaultTool === "services"
  ) {
    return defaultTool;
  }

  if (defaultTool === "browser") {
    const primary = primaryWebService(target);
    return primary ? (`browser:${primary.id}` as const) : "dashboard";
  }

  if (defaultTool?.startsWith("browser:")) {
    const serviceId = defaultTool.slice("browser:".length);
    return webServiceById(target, serviceId)
      ? (`browser:${serviceId}` as const)
      : "dashboard";
  }

  if (defaultTool && webServiceById(target, defaultTool)) {
    return `browser:${defaultTool}`;
  }

  return "dashboard";
}

export const browserServices = webServices;
export const browserServiceById = webServiceById;
export const browserSurfaceById = webSurfaceById;
export const primaryBrowserService = primaryWebService;
export const primaryBrowserSurface = primaryWebSurface;
export const backgroundPrerenderBrowserServices = backgroundPrerenderWebServices;
