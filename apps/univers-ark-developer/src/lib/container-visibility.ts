export function isHostContainer(container: { kind: string }): boolean {
  return container.kind === "host";
}

export function visibleContainers<T extends { kind: string }>(
  containers: readonly T[],
): T[] {
  return containers.filter((container) => !isHostContainer(container));
}

export function isMachineHostTarget(target: {
  containerKind?: string;
  machineId?: string;
}): boolean {
  return target.containerKind === "host" && Boolean(target.machineId);
}
