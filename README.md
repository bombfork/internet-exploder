# Internet Exploder

A minimal, fast, private-by-default web browser built from scratch in Rust.

## Goals

- **Performance** — GPU-accelerated rendering, arena-allocated DOM, zero-copy where possible
- **Low memory** — no background prefetch, no speculative loading, lean process model
- **Secure** — multi-process sandboxing, pure Rust stack (no C/C++ dependencies for core functionality)
- **Private by default** — no telemetry, no completion, no prefetch, no tracking accommodations
- **Portable** — Linux, macOS, Windows from day one via wgpu and winit
- **Latest standards only** — no legacy quirks modes, no deprecated elements, no vendor prefixes

## Non-goals

- Developer tools
- Extension ecosystem
- Legacy website compatibility
- Address bar suggestions/completion
- Spell checking

## Building

Requires [mise](https://mise.jdx.dev/) for task running.

```bash
mise install        # Install toolchain
mise run build      # Build
mise run test       # Test
mise run run        # Launch
```

## Architecture

```
ie-shell        Browser chrome, window management, event loop
ie-net          HTTP/1.1, HTTP/2 networking (hyper + rustls)
ie-html         WHATWG HTML parser
ie-css          CSS parser and style engine
ie-dom          Arena-allocated DOM tree
ie-js           JavaScript engine (Boa)
ie-layout       Layout engine (block, inline, flex, grid)
ie-render       GPU rendering (wgpu)
ie-sandbox      Multi-process sandboxing and IPC
```

## License

[GPL-3.0-or-later](LICENSE)
