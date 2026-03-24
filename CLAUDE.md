# CLAUDE.md — Internet Exploder

## What This Is

A web browser from scratch in Rust. Performance, low memory, secure, private by default. Latest web standards only — no legacy/quirks support. GPL-3.0-or-later.

## Architecture

Multi-process with sandboxing:
- **Browser process** (`ie-shell`): window, tabs, navigation, bookmarks
- **Renderer process** (per tab): HTML, CSS, layout, JS, painting
- **Network process** (singleton): all HTTP/TLS traffic

Headless mode is a first-class requirement — the browser must be fully operable without a window, for e2e testing and automation.

### Crate Map

| Crate | Purpose |
|-------|---------|
| `ie-shell` | Main binary — browser chrome, event loop (winit), headless mode |
| `ie-net` | HTTP/1.1, HTTP/2 via hyper + rustls |
| `ie-html` | WHATWG HTML parser (tokenizer + tree builder) |
| `ie-css` | CSS parser + style resolution |
| `ie-dom` | Arena-allocated DOM tree |
| `ie-js` | JavaScript via Boa engine |
| `ie-wasm` | WebAssembly execution via wasmtime |
| `ie-layout` | Layout engine (block, inline, flex, grid) |
| `ie-render` | GPU rendering via wgpu |
| `ie-sandbox` | Process spawning, sandboxing, IPC |

### Dependency Flow

```
ie-shell
├── ie-render → ie-layout → ie-dom, ie-css → ie-dom
├── ie-html → ie-dom
├── ie-js → ie-dom, ie-wasm
├── ie-net
└── ie-sandbox
```

## UX Rules

Maximum viewport, minimum chrome:
- No visible menu bar
- Tabs hidden while browsing (shortcut to reveal)
- Address bar on demand only
- Bookmarks via shortcut, no persistent bar
- No background prefetch/preload
- No address bar completion
- No spell checking

## Roadmap

### Phase 1 — Browser infrastructure (current)

Everything that is NOT rendering a web page:

- `ie-shell`: window management, headless mode, keyboard-driven UI, tab lifecycle, bookmarks storage, address bar overlay
- `ie-net`: HTTP client, TLS, request/response pipeline
- `ie-sandbox`: multi-process spawning, IPC protocol, OS-level sandboxing
- `ie-dom`: data structures (arena allocator, node types, tree operations)
- e2e test harness: headless browser driving, assertions on navigation and state

### Phase 2 — Web page rendering

Everything that IS rendering a web page:

- `ie-html`: WHATWG tokenizer + tree builder
- `ie-css`: CSS parsing, cascade, selector matching, computed styles
- `ie-layout`: block, inline, flex, grid layout
- `ie-render`: wgpu paint pipeline
- `ie-js`: Boa integration, DOM bindings, event dispatch
- `ie-wasm`: WebAssembly execution via wasmtime, JS↔Wasm interop

## Testing

Three levels, all run via `mise run test`:

- **Unit tests**: per-crate `#[test]` modules. Test internals in isolation.
- **Integration tests**: per-crate `tests/` directories. Test crate public APIs across module boundaries.
- **E2E tests**: top-level `tests/` directory. Launch the browser in headless mode, navigate to pages (local test fixtures or test server), assert on DOM state, network activity, and tab/bookmark behavior. The headless mode must be fully functional from day one to enable this.

## Build & Test

```bash
mise run build       # Build all crates
mise run test        # Run all tests (unit + integration + e2e)
mise run fmt:check   # Check formatting
mise run lint:check  # Clippy checks
mise run check       # All of the above
mise run run         # Launch the browser
```

## Git Rules

These rules have NO exceptions:

- **Never bypass pre-commit hooks** (`--no-verify` is forbidden)
- **Never force push** (`--force`, `--force-with-lease` are forbidden)
- **Only amend commits that have not been pushed** — once pushed, create a new commit instead

## Commit Messages

Use short conventional commits referencing the GitHub issue:

```
feat(ie-dom): add tree traversal iterators #4
fix(ie-net): handle redirect loop edge case #5
test(ie-shell): CLI parsing unit tests #6
refactor(ie-css): split style.rs into modules #22
chore: update dependencies
```

Format: `type(scope): short description #issue`

Types: `feat`, `fix`, `test`, `refactor`, `chore`, `docs`

## Code Style

- Keep comments short — one line when possible, no prose
- No comments for self-evident code

## Key Design Decisions

- **Latest standards only**: no quirks mode, no legacy HTML elements, no vendor-prefixed CSS
- **Boa for JS**: pure Rust, no C/C++ FFI
- **wasmtime for WebAssembly**: pure Rust, Bytecode Alliance, sandboxed execution
- **wgpu for rendering**: GPU-accelerated, cross-platform; browser chrome uses the same pipeline as page content
- **rustls over OpenSSL**: pure Rust TLS, no system dependency
- **Arena-allocated DOM**: cache-friendly, low allocation overhead
- **No preloading/prefetching**: every network request is explicit
- **Headless from day one**: enables e2e testing and CI without a display
