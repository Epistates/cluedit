import { invoke } from "@tauri-apps/api/core";
import type {
  ProjectInfo,
  ConversationMetadata,
  Conversation,
  SearchResult,
  ExportFormat,
  FastSearchResult,
  IndexStats,
} from "./types";

export async function listProjects(): Promise<ProjectInfo[]> {
  return invoke("list_projects");
}

export async function listConversations(
  projectPath: string
): Promise<ConversationMetadata[]> {
  return invoke("list_conversations", { projectPath });
}

export async function readConversation(
  filePath: string
): Promise<Conversation> {
  return invoke("read_conversation", { filePath });
}

export async function searchConversations(
  query: string,
  projectPaths: string[],
  caseSensitive: boolean = false,
  useRegex: boolean = false
): Promise<SearchResult[]> {
  return invoke("search_conversations", {
    query,
    projectPaths,
    caseSensitive,
    useRegex,
  });
}

export async function exportConversation(
  filePath: string,
  format: ExportFormat
): Promise<string> {
  return invoke("export_conversation", { filePath, format });
}

export async function getConversationMetadata(
  filePath: string
): Promise<ConversationMetadata> {
  return invoke("get_conversation_metadata", { filePath });
}

export async function findParentConversation(
  parentUuid: string
): Promise<string | null> {
  return invoke("find_parent_conversation", { parentUuid });
}

// ============================================================================
// FAST SEARCH API (Tantivy-based)
// ============================================================================

/**
 * Start background indexing of all conversations
 * Emits "indexing-progress" events that can be listened to
 */
export async function startIndexing(projectPaths: string[]): Promise<void> {
  return invoke("start_indexing", { projectPaths });
}

/**
 * Fast search using Tantivy full-text index
 * Returns results instantly (non-blocking)
 */
export async function fastSearch(
  query: string,
  limit?: number,
  fuzzy?: boolean
): Promise<FastSearchResult[]> {
  return invoke("fast_search", { query, limit, fuzzy });
}

/**
 * Get search index statistics
 */
export async function getIndexStats(): Promise<IndexStats> {
  return invoke("get_index_stats");
}
