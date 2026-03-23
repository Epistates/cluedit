# CluEdit

Desktop application for browsing, searching, backing up, branching, and exporting Claude Code conversations as production-ready LLM training data. Built with [Tauri](https://tauri.app) + [SvelteKit](https://svelte.dev).

## Features

### Conversation Browser
- Browse all Claude Code projects and conversations from `~/.claude/projects/`
- Rich metadata: message counts, token usage, cost estimates, tools used, files modified
- Full-text search powered by [Tantivy](https://github.com/quickwit-oss/tantivy)
- Filter by date range, sort by modified/created/size
- Live refresh via filesystem watcher — new conversations appear automatically

### Conversation Viewer
- Formatted message display with syntax-highlighted code blocks ([Shiki](https://shiki.style))
- Expandable tool call details with input/output
- In-conversation search
- Stats dashboard with token counts and estimated costs
- Continuation chain navigation between related conversations

### Backup & Branching
- **Backup** conversations at any point — full or truncated at a specific event
- **Restore** from any backup with automatic safety backup of current state
- **Branch** conversations — duplicate with all UUIDs regenerated so Claude treats the branch as independent
- **Branch from any message** — click "branch here" on any message to fork from that point
- Branch from backups to create new conversations from saved checkpoints

### Training Data Export
Export conversations in formats ready for LLM fine-tuning:

| Format | Mode | Description |
|--------|------|-------------|
| **ChatML** | Conversational | OpenAI fine-tuning JSONL — text-only exchanges, no tool noise |
| **ChatML + Tools** | Agentic | Structured `tool_calls` + `role: "tool"` for training tool-calling models |
| **ShareGPT** | Conversational | `human`/`gpt` turn pairs for open-source fine-tuning frameworks |
| **Alpaca** | Conversational | Instruction/output pairs for instruction-tuning |

Training exports include:
- Automatic stripping of Claude-specific XML tags and system artifacts
- Filtering of low-value narration messages
- Merging consecutive same-role messages for valid alternating format
- Token-aware chunking — long conversations split into segments under 16k tokens
- Proper OpenAI function-calling schema with `tools` array for agentic mode

Also exports to JSON, Markdown, and plain text.

### Export Scope
- **Single conversation** — from the conversation viewer
- **All conversations in a project** — from the toolbar
- **All projects** — from the sidebar

## Getting Started

### Prerequisites
- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/)

### Setup
```bash
pnpm install
pnpm tauri dev
```

### Build
```bash
pnpm tauri build
```

## Project Structure
```
src/                          # SvelteKit frontend
  lib/
    components/               # Svelte 5 components
    stores/                   # Svelte stores
    api.ts                    # Tauri IPC bindings
    types.ts                  # TypeScript types
src-tauri/                    # Rust backend
  src/
    backup_service.rs         # Backup, restore, branch with ID remapping
    content_sanitizer.rs      # Training data cleaning, chunking, tool schemas
    conversation_service.rs   # Core I/O, metadata, export (8 formats)
    conversation_analyzer.rs  # Metadata extraction from JSONL events
    search_indexer.rs         # Tantivy full-text search index
    file_watcher.rs           # Live filesystem monitoring
    commands.rs               # Tauri IPC command handlers
    models.rs                 # Shared data types
```

## Tech Stack

- **Framework**: Tauri 2.x
- **Backend**: Rust (serde, tantivy, regex, tokio, uuid)
- **Frontend**: SvelteKit 2.x, Svelte 5, TypeScript
- **Styling**: Tailwind CSS 4, Bits UI, Lucide icons
- **Code Highlighting**: Shiki

## License

MIT
