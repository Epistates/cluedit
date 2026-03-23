import { invoke } from "@tauri-apps/api/core";
import type {
  ProjectInfo,
  ConversationMetadata,
  Conversation,
  SearchResult,
  ExportFormat,
  FastSearchResult,
  IndexStats,
  BackupInfo,
  BranchResult,
  ExportAllResult,
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

// ============================================================================
// BACKUP & BRANCH API
// ============================================================================

/** Create a full backup of a conversation */
export async function createBackup(
  filePath: string,
  label: string
): Promise<BackupInfo> {
  return invoke("create_backup", { filePath, label });
}

/** Create a backup truncated at a specific event index (0-based, inclusive) */
export async function createBackupAtEvent(
  filePath: string,
  eventIndex: number,
  label: string
): Promise<BackupInfo> {
  return invoke("create_backup_at_event", { filePath, eventIndex, label });
}

/** List all backups for a specific conversation */
export async function listBackups(
  conversationId: string
): Promise<BackupInfo[]> {
  return invoke("list_backups", { conversationId });
}

/** List all backups across all conversations */
export async function listAllBackups(): Promise<BackupInfo[]> {
  return invoke("list_all_backups");
}

/** Restore a conversation from a backup. Returns the auto-created safety backup. */
export async function restoreBackup(backupId: string): Promise<BackupInfo> {
  return invoke("restore_backup", { backupId });
}

/** Branch a conversation: duplicate with regenerated IDs, optionally truncated */
export async function branchConversation(
  sourcePath: string,
  truncateAtEvent?: number
): Promise<BranchResult> {
  return invoke("branch_conversation", { sourcePath, truncateAtEvent });
}

/** Branch from a backup: create new conversation from backup with regenerated IDs */
export async function branchFromBackup(
  backupId: string
): Promise<BranchResult> {
  return invoke("branch_from_backup", { backupId });
}

/** Delete a backup and its file */
export async function deleteBackup(backupId: string): Promise<void> {
  return invoke("delete_backup", { backupId });
}

// ============================================================================
// BULK EXPORT API
// ============================================================================

/** Export a single conversation directly to a file (backend writes, bypasses FS scope) */
export async function exportConversationToFile(
  filePath: string,
  format: ExportFormat,
  outputPath: string
): Promise<void> {
  return invoke("export_conversation_to_file", { filePath, format, outputPath });
}

/** Export all conversations in the given projects to a file or directory.
 *  Pass empty projectPaths to export ALL projects. */
export async function exportAllConversations(
  projectPaths: string[],
  format: ExportFormat,
  outputPath: string
): Promise<ExportAllResult> {
  return invoke("export_all_conversations", { projectPaths, format, outputPath });
}
