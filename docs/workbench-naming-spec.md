# Workbench Naming Specification

## Purpose

This document defines the product-facing naming model for `univers-machine`.

The goal is to shift the primary user mental model away from infrastructure terms like "machine" and "container" and toward AI work environments.

## Core Concepts

### Workbench

`Workbench` is the primary user-facing object.

Definition:
- A Workbench is an AI working environment.
- It is the environment a user opens, switches, inspects, and manages.
- It may be implemented by a container, VM, remote shell target, or another runtime, but those are implementation details.

Rules:
- Use `Workbench` as the top-level product concept.
- Use `Workbench` in primary navigation, creation flows, details pages, and status displays.
- Do not replace `Workbench` with `Container`, `Target`, or `Profile` in user-facing copy.

### Provider

`Provider` is the source category a Workbench comes from.

Definition:
- A Provider groups Workbenches by origin or access model.
- Examples: `Machines`, `AWS`, `Azure`.

Rules:
- Use `Provider` to organize and group Workbenches.
- A Provider may expose Workbenches directly.
- A Provider may also expose an intermediate host layer.

### Host

`Host` is the optional execution location where a Workbench runs.

Definition:
- A Host is the runtime location behind a Workbench when that location matters.
- In the `Machines` provider, a Host is typically a physical machine or VM.
- In other providers, Host may be hidden or absent.

Rules:
- `Host` is a secondary infrastructure concept.
- Show `Host` only when operational clarity requires it.
- Do not use `Machine` as the cross-provider term.

## Canonical Object Model

The canonical model is:

`Provider -> Host? -> Workbench -> Session / Task`

Notes:
- `Host` is optional.
- `Machines` usually has a host layer.
- `AWS` and `Azure` may attach Workbenches directly under the provider.

Examples:

- `Machines -> mac-studio -> iOS Workbench`
- `Machines -> ubuntu-dev-1 -> Backend Workbench`
- `Azure -> Evaluation Workbench`
- `AWS -> Research Workbench`

## Naming Rules

### Preferred Product Terms

Use these terms in the UI, docs, and product discussions:

- `Workbench`
- `Provider`
- `Host`
- `Session`
- `Task`
- `Template`

Definitions:
- `Session`: an interactive shell or workspace session inside a Workbench
- `Task`: a unit of AI work executed in a Workbench
- `Template`: a preset used to create a Workbench

### Terms To Avoid In Primary UI

These may still exist internally, but should not be primary user-facing concepts:

- `Machine`
- `Container`
- `Target`
- `Profile`
- `Runtime`
- `SSH Target`

Usage rule:
- If a user is choosing, opening, or managing an environment, call it a `Workbench`.
- If the system is grouping where environments come from, call that a `Provider`.
- If the system is exposing where an environment runs, call that a `Host`.

## Hierarchy Guidelines

### Top-Level Navigation

Recommended top-level naming:

- `All Workbenches`
- `Providers`
- `Templates`
- `Settings`

Avoid using these as top-level navigation labels:

- `Machines`
- `Containers`
- `Targets`

### Provider Tree

Recommended provider tree:

- `Machines`
  - `mac-studio`
  - `ubuntu-dev-1`
- `AWS`
- `Azure`

Interpretation:
- `Machines` is a provider name.
- `mac-studio` and `ubuntu-dev-1` are hosts.
- Workbenches appear under hosts when applicable.
- Other providers may expose Workbenches directly.

## Page Naming

Recommended page names:

- `All Workbenches`
- `Workbench Details`
- `Workbench Settings`
- `Create Workbench`
- `Providers`
- `Sessions`
- `Tasks`
- `Templates`

Avoid:

- `Machine Management`
- `Container Management`
- `Add Target`
- `Profile Management`

## Action Naming

Recommended action labels:

- `Create Workbench`
- `Open Workbench`
- `Start Session`
- `Run Task`
- `Change Provider`
- `Move to Host`

Avoid:

- `Add Target`
- `Create Container`
- `Attach Machine`

## List And Field Naming

Recommended Workbench list fields:

- `Name`
- `Provider`
- `Host`
- `Status`
- `Sessions`
- `Last Active`

Only show infrastructure-specific fields in advanced or diagnostic views:

- `Container ID`
- `VM Type`
- `SSH Profile`
- `Runtime`

## Mapping From Existing Terms

Map current terms to the new model as follows:

- `machine` -> `host`
- `container` -> `workbench` in product copy, or `runtime instance` internally
- `target` -> `workbench`
- `profile` -> `template` or `workbench config`, depending on meaning
- `orbstack` / `lxd` / `docker` -> `runtime` internally

Important rule:
- `Machine` is not the universal abstraction.
- `Machines` is a provider category.
- Within that provider, individual machines are hosts.

## Product Language Summary

The product should communicate the following model:

- A `Workbench` is the environment.
- A `Provider` is where it comes from.
- A `Host` is where it runs, when that matters.

This should be the default language in UI copy, documentation, and roadmap discussions.
