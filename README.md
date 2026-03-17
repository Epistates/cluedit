# ClaudeEdit

Enterprise-grade Tauri application for managing, searching, and analyzing Claude Code conversation history.

## Features

- **Project Management**: Browse all Claude Code projects and their conversations
- **Conversation Viewer**: Inspect detailed conversation history with all events
- **Full-Text Search**: Search across all conversations with regex support
- **Export Capabilities**: Export conversations to JSON, Markdown, or plain text
- **Modern UI**: Clean, dark-mode interface with responsive design
- **Fast Performance**: Built with Rust backend for efficient file operations

## Architecture

### Backend (Rust)
- **Models**: Type-safe data structures for conversations, metadata, and search results
- **ConversationService**: Core service for file system operations, JSONL parsing, and search
- **Commands**: Tauri IPC commands exposed to frontend
- **Error Handling**: Comprehensive error types with proper serialization

### Frontend (Svelte + TypeScript)
- **Components**:
  - `Sidebar`: Project navigation
  - `ConversationList`: Conversation browser with sorting
  - `ConversationDetail`: Full event viewer with export
  - `SearchView`: Advanced search with context
  - `Toolbar`: View switching and controls
- **Stores**: Reactive state management with derived stores
- **API**: Type-safe wrapper around Tauri commands

## Data Location

The application reads from `~/.claude/`:
- `projects/<project>/` - Project-specific conversations (UUID.jsonl files)
- `history.jsonl` - Main conversation history
- `file-history/` - File modification history
- `todos/` - Todo tracking

## Development

```bash
# Install dependencies
pnpm install

# Run in development mode
pnpm run tauri dev

# Build for production
pnpm run tauri build

# Check TypeScript/Svelte
pnpm run check

# Build frontend only
pnpm run build
```

## Tech Stack

- **Framework**: Tauri 2.x
- **Backend**: Rust (serde, walkdir, regex, chrono)
- **Frontend**: SvelteKit 2.x + TypeScript
- **Build**: Vite 6.x
- **Package Manager**: pnpm

## Usage

1. Launch the application
2. Projects load automatically from `~/.claude/projects/`
3. Select a project to view its conversations
4. Click a conversation to view full details
5. Use the Search tab to find specific content across all conversations
6. Export conversations using the Export button in detail view

## Export Formats

- **JSON (Pretty)**: Formatted JSON with indentation
- **JSON (Compact)**: Minified JSON
- **Markdown**: Readable markdown with headers and code blocks
- **Text**: Plain text with separators

## Development Notes

- All file operations happen in Rust for security and performance
- JSONL files are parsed line-by-line to handle large conversations
- Search uses regex for powerful pattern matching
- UI follows VS Code dark theme conventions
- Fully type-safe frontend-backend communication

## Future Enhancements

- Tag and categorize conversations
- Advanced filtering (date ranges, size, event types)
- Conversation analytics and statistics
- Import/merge conversations
- Diff view between conversation snapshots
- Full-text indexing for faster search
- Conversation notes and annotations

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).
