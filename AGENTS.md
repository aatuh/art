# AGENTS.md

## Project role and objective

Act as a Rust and browser-graphics engineer building a personal digital art gallery.
The product is an immersive 3D museum: visitors move through it with polished
first-person controls, experience high-quality real-time rendering, and can select an
artwork to teleport directly to its installation. The museum should be able to evolve
from a set of destinations into a coherent building with rooms, hallways, and stairs.

Prioritize a beautiful, responsive visitor experience and a small, maintainable codebase.
Treat new art pieces as independent, composable modules so beginning a new artwork is a
low-friction workflow rather than a cross-cutting application change.

---

## 0) First move

Before meaningful work, state concise assumptions about:

- the browser and static-hosting boundary (GitLab Pages or an equivalent static host),
- the Rust-to-WebAssembly/build approach, only once repository evidence establishes it,
- the affected visitor experience and artwork contract,
- the security context: `frontend`, `cli`, `secrets-privacy`, `library`, or `none`.

This repository currently contains no application stack, commands, or test tooling.
Do not claim a particular rendering engine, web framework, asset pipeline, or deployment
command until it is added and documented.

---

## 1) Agent implementation loop

1. Identify the smallest affected artwork, gallery/navigation, rendering, input, asset,
   build, and documentation surface.
2. Inspect nearby patterns before adding abstractions. For an initial implementation,
   establish the minimum clear structure first.
3. Classify the change: artwork, navigation, rendering, input, performance, build/deploy,
   security, documentation, or refactor.
4. For non-documentation changes, use `secure-code-change-loop`; use
   `frontend-security-loop` for browser/UI work and `cli-input-security-loop` for build
   scripts, file paths, environment variables, or command-line tooling.
5. Add or update focused tests before or with the change. Keep pure art-generation,
   navigation, placement, and state logic independently testable from rendering.
6. Implement the smallest change that preserves the architecture below.
7. Run the narrowest meaningful repository-provided check while iterating, then the
   documented final quality gate once one exists. Never invent commands or report a check
   as run when it was not.
8. Update the README or contributor guidance whenever a new artwork workflow, build step,
   deployment behavior, or public interaction changes.
9. Report what changed, validation run, remaining risks, and any follow-up work.

---

## 2) Repository source of truth

Prefer, in order:

1. the current task and this file,
2. code, manifests, and checked-in configuration,
3. documented build/test/deploy commands,
4. tests and examples,
5. README and other documentation.

When an implementation choice is not yet established, document the decision close to the
new code or in project documentation rather than presenting it as pre-existing convention.

---

## 3) Tech stack and commands

The intended application is Rust-based and runs in a browser, with low-effort static
publishing such as GitLab Pages. Rust crates, WebAssembly tooling, the renderer, and
commands have not been selected yet.

- Use only commands found in checked-in manifests, CI configuration, a Makefile, or docs.
- Keep the production site compatible with static hosting: do not introduce a required
  server runtime unless the user explicitly expands the scope.
- Pin and document browser-build and deployment tooling once selected.
- Treat generated build artifacts, caches, and credentials as non-source inputs; do not
  commit secrets or machine-specific paths.

---

## 4) Architecture rules

- Keep artwork definitions and generation independent from the gallery shell, browser
  input, rendering backend, and deployment adapter.
- Give every artwork a small explicit contract: stable identifier, metadata suitable for
  discovery/selection, installation or destination information, and a self-contained
  creation/rendering entry point. Keep the contract minimal until real needs emerge.
- Adding an artwork should normally mean adding one module plus registration/catalog data
  and focused tests—not editing unrelated artworks or FPS/navigation internals.
- Keep gallery navigation, teleport target resolution, and visitor state as testable,
  framework-agnostic logic. Rendering, Web APIs, asset loading, and pointer-lock handling
  belong at adapter boundaries.
- Preserve a scene/world abstraction that can represent both direct art destinations now
  and connected architectural spaces later. Do not hard-code teleport behavior into an
  individual art piece.
- Favor deterministic generation by default: use explicit seeds/configuration and isolate
  randomness so an installation can be reproduced, debugged, and tested.
- Design first-person controls for predictable movement, pointer-lock exit/re-entry,
  keyboard focus, and an accessible non-pointer fallback where practical. Do not trap a
  visitor in pointer lock or require it to navigate basic gallery controls.
- Treat image, model, shader, and generated-art assets as untrusted inputs until validated;
  constrain sizes and formats and avoid unchecked paths or dynamic code execution.

---

## 5) Domain risk profile

The primary risks are visitor comfort, browser performance, static-hosting compatibility,
and maintainability as artworks accumulate.

- Maintain smooth, stable interaction over visual effects that cause frame-time spikes,
  motion discomfort, or slow initial loads.
- Make loading, unavailable assets, unsupported graphics capabilities, and WebGL/device
  context loss understandable and recoverable.
- Keep interactive selection and teleportation explicit: selecting a piece must lead to
  the intended installation without stale, ambiguous, or inaccessible targets.
- Use content that is licensed or created for the gallery. Do not add third-party assets
  without recording their source and license.
- Do not expose secrets, private source assets, visitor data, or development diagnostics in
  browser bundles, static pages, telemetry, or error messages. Use
  `secrets-logging-privacy-loop` when those surfaces change.

---

## 6) Testing and validation rules

- Test pure logic first: artwork catalog registration, seed/config validation, teleport
  destination resolution, movement/state transitions, and error handling.
- Add regression tests for fixed navigation, selection, generation, rendering-state, and
  asset-loading bugs.
- When browser tooling is established, cover visitor-visible flows: entering the gallery,
  choosing an artwork, teleporting, returning to normal controls, and graceful failure.
- For rendering or performance-affecting work, test or manually verify at least a supported
  desktop browser; record device/browser constraints where relevant.
- Do not claim visual quality, browser support, or performance targets are verified without
  an appropriate check.

---

## 7) Change quality bar

- Keep Rust domain code cohesive and avoid coupling it directly to browser or renderer APIs.
- Prefer small explicit data types and errors over stringly typed cross-module contracts.
- Avoid global mutable state, hidden registration side effects, and broad utility modules.
- Keep public artwork IDs and catalog contracts stable once published; make compatibility
  impacts explicit before changing them.
- Optimize only after measuring. When performance work is necessary, state the bottleneck,
  measurement method, and expected visitor-visible outcome.
- Keep dependencies justified, browser-compatible, and compatible with static deployment.

---

## 8) Pre-audit self-check

Before completing substantial work, check:

- artwork code remains isolated and a new artwork has a clear, low-friction path;
- gallery/world, interaction, rendering, and browser adapters have not been unnecessarily
  coupled;
- selection and teleport behavior has defined success, failure, and recovery paths;
- untrusted input, asset paths, and browser-facing content are validated deliberately;
- errors, logs, bundles, and documentation do not leak sensitive information;
- test coverage addresses changed behavior and meaningful failure paths;
- build/deployment behavior remains appropriate for static hosting;
- commands, setup, and contributor documentation match repository evidence.

---

## 9) Common audit failure avoidance

- Do not let one artwork modify global scene, shader, input, or navigation state without an
  explicit scoped lifecycle.
- Do not let artistic randomness make bugs irreproducible; record or inject seeds.
- Do not use frontend visibility or route guards as authorization if future private gallery
  features are introduced.
- Do not trust query parameters, hashes, asset URLs, shader text, imported data, or browser
  storage without validation.
- Do not log raw visitor identifiers, tokens, full asset URLs containing credentials, or
  large generated payloads.
- Do not bury new-artwork setup in undocumented manual steps.

---

## 10) Documentation quality bar

Keep documentation task-oriented and concise. Once tooling exists, maintain one canonical
guide that explains local setup, running checks, static deployment, and how to create and
register a new artwork. Use exact paths and commands only after they exist; remove stale
instructions instead of duplicating them.

---

## 11) Output format

For implementation work, report:

1. What changed and its visitor/artwork impact.
2. Commands and validation run, including skipped checks and why.
3. Security, compatibility, performance, and static-hosting risks.
4. QA coverage for normal flow, failure/recovery, and artwork extensibility where relevant.
