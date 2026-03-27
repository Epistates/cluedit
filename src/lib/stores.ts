import { writable, derived } from "svelte/store";
import type {
  ProjectInfo,
  ConversationMetadata,
  Conversation,
  SearchResult,
  Provider,
} from "./types";

// OS username (for redaction in filenames)
export const osUsername = writable<string | null>(null);

// Provider store
export const activeProvider = writable<Provider>("Claude");
export const availableProviders = writable<Provider[]>(["Claude"]);

// Projects store
export const projects = writable<ProjectInfo[]>([]);
export const selectedProject = writable<ProjectInfo | null>(null);

// Conversations store
export const conversations = writable<ConversationMetadata[]>([]);
export const selectedConversation = writable<Conversation | null>(null);

// Search store
export const searchQuery = writable<string>("");
export const searchResults = writable<SearchResult[]>([]);
export const isSearching = writable<boolean>(false);

// Filter/sort options
export const sortBy = writable<"modified" | "created" | "size">("modified");
export const sortOrder = writable<"asc" | "desc">("desc");
export const showEmptyConversations = writable<boolean>(false);
export const dateFilterFrom = writable<string>("");
export const dateFilterTo = writable<string>("");

// UI state
export const sidebarCollapsed = writable<boolean>(false);
export const viewMode = writable<"list" | "detail" | "search">("list");
export const commandPaletteOpen = writable<boolean>(false);

// Filtered and sorted conversations
export const filteredConversations = derived(
  [conversations, sortBy, sortOrder, showEmptyConversations, dateFilterFrom, dateFilterTo],
  ([$conversations, $sortBy, $sortOrder, $showEmptyConversations, $dateFilterFrom, $dateFilterTo]) => {
    // Filter out empty conversations if toggle is off
    let filtered = $conversations;
    if (!$showEmptyConversations) {
      filtered = $conversations.filter(
        (conv) => conv.total_message_count > 0 || conv.title != null
      );
    }

    // Date range filtering
    if ($dateFilterFrom) {
      const fromTs = new Date($dateFilterFrom).getTime() / 1000;
      filtered = filtered.filter((conv) => conv.modified >= fromTs);
    }
    if ($dateFilterTo) {
      const toTs = new Date($dateFilterTo).getTime() / 1000 + 86400; // end of day
      filtered = filtered.filter((conv) => conv.modified <= toTs);
    }

    // Sort
    const sorted = [...filtered].sort((a, b) => {
      let comparison = 0;
      switch ($sortBy) {
        case "modified":
          comparison = a.modified - b.modified;
          break;
        case "created":
          comparison = a.created - b.created;
          break;
        case "size":
          comparison = a.size_bytes - b.size_bytes;
          break;
      }
      return $sortOrder === "asc" ? comparison : -comparison;
    });
    return sorted;
  }
);
