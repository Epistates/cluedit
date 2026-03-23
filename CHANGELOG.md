# Changelog

## [0.1.0] - 2026-03-23

### Added
- **Conversation Browser** — browse all Claude Code projects and conversations with rich metadata (message counts, token usage, cost estimates, tools used, files modified)
- **Full-Text Search** — Tantivy-powered search index with background indexing and instant results
- **Conversation Viewer** — formatted messages with Shiki syntax highlighting, expandable tool calls, in-conversation search, and stats dashboard
- **Backup System** — create, restore, and delete conversation backups with automatic safety backups before restore
- **Conversation Branching** — duplicate conversations with full UUID remapping; branch from any message or backup
- **Training Data Export** — 5 export formats for LLM fine-tuning:
  - ChatML (conversational, text-only) with token-aware chunking
  - ChatML + Tools (agentic, structured tool_calls and role:"tool" messages)
  - ShareGPT (human/gpt turn pairs)
  - Alpaca (instruction/output pairs)
  - Plus JSON, Markdown, and plain text
- **Content Sanitizer** — strips Claude-specific XML tags, filters command/system artifacts, merges consecutive same-role messages, estimates tokens, chunks long conversations
- **Export Scope** — export single conversations, all in a project, or all projects at once
- **Live Refresh** — filesystem watcher detects new/changed conversations within 500ms
- **Continuation Chaining** — navigate between linked conversations via logicalParentUuid
- **Command Palette** — Cmd+K quick navigation
- **Filtering & Sorting** — date range filters, sort by modified/created/size, toggle empty conversations
