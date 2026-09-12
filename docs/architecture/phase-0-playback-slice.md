# Phase 0 Playback Slice

## Scope and boundaries

This slice adds transport for the currently verified WAV without changing the
`VocalProject` schema or treating playback state as semantic data. `vocal-app`
owns a provider-independent `PlaybackEngine` port and identity-scoped playback
session. The Tauri adapter supplies CPAL 0.18.2; the frontend only sends coarse
commands and renders the returned projection.

Stable application operations are `load_playback`, `play`, `pause`, `seek`,
`stop`, and `playback_status`. Every operation is scoped by project id,
generation, and track id. Loading is allowed only for a `Ready` track, resolves
the current project-relative source, verifies its stored size/hash, and decodes
a fresh bounded PCM buffer. It does not reuse or publish an analysis artifact or
analysis-runtime cache entry.

## Invariants

- `VocalProject` remains the sole semantic truth; transport position, device
  stream, decoded playback buffer, and errors are replaceable application state.
- Project replacement/close and every import, analysis, or Relink job stop and
  unbind the previous stream before their state transition.
- A missing or changed source discovered during load invalidates only the
  derived source projection. It does not dirty or rewrite the project.
- Playback publication rechecks project generation, track identity, source
  hash, and `Ready` status after decoding, so a stale load cannot bind to a new
  project.
- The CPAL callback captures immutable PCM and uses atomics for play state and
  frame position. It performs no logging, locking, analysis, I/O, or allocation.
- The first slice preserves the WAV sample rate and one/two source channels. It
  requires a compatible device configuration, duplicates mono to output
  channels, and leaves extra output channels silent. Resampling, device choice,
  loop regions, and latency compensation remain later playback work.

## Verification

Application tests inject a deterministic engine to cover load/play/pause/seek/
stop, identity rejection, and transition stop behavior. Tauri MockRuntime tests
cover the IPC projection. The CPAL callback has a pure buffer/cursor/end test;
frontend tests cover initial load from the viewport, polling, pause, seek, stop,
and timer disposal. Dependency metadata and the desktop lockfile include CPAL
and all target-specific transitive licenses.
