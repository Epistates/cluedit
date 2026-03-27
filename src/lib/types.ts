// ============================================================================
// Claude Code JSONL Event Types
// ============================================================================

export interface ContentBlockText {
  type: "text";
  text: string;
}

export interface ContentBlockToolUse {
  type: "tool_use";
  id: string;
  name: string;
  input: Record<string, unknown>;
}

export interface ContentBlockToolResult {
  type: "tool_result";
  tool_use_id: string;
  content?: unknown;
  is_error?: boolean;
}

export type ContentBlock =
  | ContentBlockText
  | ContentBlockToolUse
  | ContentBlockToolResult
  | { type: string; [key: string]: unknown };

export type MessageContent = string | ContentBlock[];

export interface ApiMessage {
  role: string;
  content: MessageContent;
  model?: string;
  id?: string;
  stop_reason?: string | null;
  stop_sequence?: string | null;
  usage?: Record<string, unknown>;
}

export interface UserEvent {
  type: "user";
  message: ApiMessage;
  uuid?: string;
  parentUuid?: string | null;
  sessionId?: string;
  timestamp?: string;
  isMeta?: boolean;
  isSidechain?: boolean;
  cwd?: string;
  version?: string;
  gitBranch?: string;
  slug?: string;
  logicalParentUuid?: string | null;
  promptId?: string;
  permissionMode?: string;
  isCompactSummary?: boolean;
  agentId?: string;
  sourceToolUseId?: string;
  sourceToolAssistantUuid?: string;
  toolUseResult?: unknown;
  [key: string]: unknown;
}

export interface AssistantEvent {
  type: "assistant";
  message: ApiMessage;
  uuid?: string;
  parentUuid?: string | null;
  sessionId?: string;
  timestamp?: string;
  isMeta?: boolean;
  isSidechain?: boolean;
  requestId?: string;
  cwd?: string;
  version?: string;
  slug?: string;
  agentId?: string;
  sourceToolUseId?: string;
  sourceToolAssistantUuid?: string;
  toolUseResult?: unknown;
  isApiErrorMessage?: boolean;
  error?: unknown;
  thinkingMetadata?: unknown;
  logicalParentUuid?: string | null;
  [key: string]: unknown;
}

export interface ProgressEvent {
  type: "progress";
  data?: Record<string, unknown>;
  parentToolUseID?: string;
  toolUseID?: string;
  uuid?: string;
  timestamp?: string;
  agentId?: string;
  sessionId?: string;
  cwd?: string;
  [key: string]: unknown;
}

export interface SystemEvent {
  type: "system";
  subtype?: string;
  uuid?: string;
  timestamp?: string;
  isMeta?: boolean;
  content?: unknown;
  durationMs?: number;
  compactMetadata?: unknown;
  logicalParentUuid?: string | null;
  level?: string;
  [key: string]: unknown;
}

export interface FileHistorySnapshotEvent {
  type: "file-history-snapshot";
  messageId: string;
  snapshot: Record<string, unknown>;
  isSnapshotUpdate: boolean;
}

export interface SummaryEvent {
  type: "summary";
  summary?: string;
  [key: string]: unknown;
}

export interface QueueOperationEvent {
  type: "queue-operation";
  operation?: string;
  content?: string;
  timestamp?: string;
  [key: string]: unknown;
}

export interface PrLinkEvent {
  type: "pr-link";
  sessionId?: string;
  prNumber?: number;
  prUrl?: string;
  prRepository?: string;
  timestamp?: string;
  [key: string]: unknown;
}

export interface UnknownEvent {
  type: string;
  [key: string]: unknown;
}

export type ConversationEvent =
  | UserEvent
  | AssistantEvent
  | ProgressEvent
  | SystemEvent
  | FileHistorySnapshotEvent
  | SummaryEvent
  | QueueOperationEvent
  | PrLinkEvent
  | UnknownEvent;

// ============================================================================
// Helper functions
// ============================================================================

export function isUserEvent(event: ConversationEvent): event is UserEvent {
  return event.type === "user";
}

export function isAssistantEvent(
  event: ConversationEvent
): event is AssistantEvent {
  return event.type === "assistant";
}

export function isChatMessage(event: ConversationEvent): boolean {
  return (
    (isUserEvent(event) || isAssistantEvent(event)) && !event.isMeta
  );
}

export function extractText(content: MessageContent): string {
  if (typeof content === "string") return content;
  return content
    .filter((b): b is ContentBlockText => b.type === "text")
    .map((b) => b.text)
    .join("\n\n");
}

export function getToolUses(
  content: MessageContent
): ContentBlockToolUse[] {
  if (typeof content === "string") return [];
  return content.filter(
    (b): b is ContentBlockToolUse => b.type === "tool_use"
  );
}

// ============================================================================
// Application Types
// ============================================================================

export interface ConversationMetadata {
  id: string;
  file_path: string;
  file_name: string;
  size_bytes: number;
  created: number;
  modified: number;
  event_count: number;
  project: string | null;
  first_message: string | null;
  title: string | null;
  summary: string | null;
  user_message_count: number;
  total_message_count: number;
  artifact_count: number;
  artifacts: string[];
  is_continuation: boolean;
  continued_from_id: string | null;
  has_compaction: boolean;
  last_user_message: string | null;
  topics: string[];
  total_input_tokens: number;
  total_output_tokens: number;
  session_count: number;
  tool_use_count: number;
  tool_names: string[];
}

export interface ProjectInfo {
  name: string;
  path: string;
  conversation_count: number;
}

export interface Conversation {
  metadata: ConversationMetadata;
  events: ConversationEvent[];
}

export interface SearchMatch {
  line_number: number;
  content: string;
  context_before: string[];
  context_after: string[];
}

export interface SearchResult {
  conversation_id: string;
  file_path: string;
  matches: SearchMatch[];
  total_matches: number;
}

export type ExportFormat = "Json" | "JsonPretty" | "Markdown" | "Text" | "ChatML" | "ChatMLTools" | "ShareGPT" | "Alpaca";

export type Provider = "Claude" | "Codex";

export interface ProviderInfo {
  name: string;
  provider: Provider;
  available: boolean;
}

// Fast search types (Tantivy-based)
export interface FastSearchResult {
  conversation_id: string;
  file_path: string;
  title: string | null;
  project: string | null;
  snippet: string;
  score: number;
  total_matches: number;
}

export interface IndexingProgress {
  current: number;
  total: number;
  status: string;
}

export interface IndexStats {
  indexed_conversations: number;
  num_segments: number;
}

// ============================================================================
// Backup & Branch Types
// ============================================================================

export interface BackupInfo {
  id: string;
  conversation_id: string;
  original_file_path: string;
  backup_file_path: string;
  label: string;
  created_at: string;
  event_count: number;
  truncated_at_event: number | null;
  size_bytes: number;
  auto_backup: boolean;
}

export interface BranchResult {
  new_file_path: string;
  new_conversation_id: string;
  event_count: number;
  ids_remapped: number;
}

export interface ExportAllResult {
  conversations_exported: number;
  conversations_skipped: number;
  output_path: string;
}
