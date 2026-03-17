<script lang="ts">
  import { searchQuery, selectedConversation, viewMode } from "$lib/stores";
  import { setLoading, setError, setMessage, clearStatus } from "$lib/stores/statusStore";
  import { fastSearch, readConversation } from "$lib/api";
  import type { FastSearchResult } from "$lib/types";
  import { Search, Loader2 } from "lucide-svelte";

  let fastResults: FastSearchResult[] = $state([]);
  let isSearching = $state(false);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  function debounceSearch() {
    if (debounceTimer) clearTimeout(debounceTimer);

    if (!$searchQuery.trim()) {
      fastResults = [];
      clearStatus();
      isSearching = false;
      return;
    }

    isSearching = true;

    debounceTimer = setTimeout(() => {
      handleFastSearch();
    }, 150);
  }

  $effect(() => {
    if ($searchQuery !== undefined) {
      debounceSearch();
    }
  });

  async function handleFastSearch() {
    if (!$searchQuery.trim()) {
      fastResults = [];
      clearStatus();
      isSearching = false;
      return;
    }

    isSearching = true;
    setLoading("Searching...");

    try {
      const results = await fastSearch($searchQuery, 50, true);
      fastResults = results;

      if (results.length === 0) {
        setMessage("No matches found");
      } else {
        setMessage(`Found ${results.length} conversations`);
      }
    } catch (e) {
      console.error("Fast search failed:", e);
      setError(`Search failed: ${e}`);
      fastResults = [];
    } finally {
      isSearching = false;
    }
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      if (debounceTimer) clearTimeout(debounceTimer);
      handleFastSearch();
    }
  }

  async function openConversation(result: FastSearchResult) {
    try {
      setLoading("Opening conversation...");
      const conversation = await readConversation(result.file_path);
      selectedConversation.set(conversation);
      viewMode.set("detail");
      setMessage("Conversation opened");
    } catch (e) {
      console.error("Failed to open conversation:", e);
      setError(`Failed to open conversation: ${e}`);
    }
  }

  function escapeHtml(text: string): string {
    return text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function escapeRegex(str: string): string {
    return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }

  function highlightMatch(text: string, query: string): string {
    if (!query) return escapeHtml(text);
    const escaped = escapeHtml(text);
    const escapedQuery = escapeHtml(query);
    const regex = new RegExp(`(${escapeRegex(escapedQuery)})`, "gi");
    return escaped.replace(regex, "<mark>$1</mark>");
  }
</script>

<div class="flex-1 flex flex-col h-full overflow-hidden">
  <div class="px-6 py-4 border-b border-border-default bg-bg-base">
    <div class="flex gap-4 items-center">
      <div class="flex-1 relative">
        <Search
          size={16}
          class="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted pointer-events-none"
        />
        <input
          type="text"
          bind:value={$searchQuery}
          onkeydown={handleKeyDown}
          placeholder="Search conversations..."
          class="w-full bg-bg-elevated border border-border-default text-text-primary pl-10 pr-4 py-2.5 rounded-md text-base focus:outline-none focus:border-accent"
        />
      </div>
      <div class="flex items-center min-w-[150px]">
        {#if isSearching}
          <span class="text-accent text-sm animate-pulse">Searching...</span>
        {:else if fastResults.length > 0}
          <span class="text-success-hover text-sm">{fastResults.length} conversations</span>
        {:else if $searchQuery.trim()}
          <span class="text-text-muted text-sm">No results</span>
        {:else}
          <span class="text-text-faint text-sm">Type to search</span>
        {/if}
      </div>
    </div>
  </div>

  <div class="flex-1 overflow-y-auto p-4 bg-bg-surface">
    {#if isSearching}
      <div class="flex flex-col items-center gap-4 p-12 text-text-muted text-base">
        <Loader2 size={32} class="animate-spin text-accent" />
        <span>Searching conversations...</span>
      </div>
    {:else if fastResults.length === 0 && $searchQuery.trim()}
      <div class="flex flex-col items-center gap-2 p-12 text-text-muted text-base">
        <span>No matches found for "{$searchQuery}"</span>
        <p class="text-sm text-text-faint">Try different keywords</p>
      </div>
    {:else if !$searchQuery.trim()}
      <div class="flex flex-col items-center gap-2 p-12 text-text-muted text-base">
        <span>Full-text search powered by Tantivy</span>
        <p class="text-sm text-text-faint">Start typing to search across all conversations</p>
      </div>
    {:else if fastResults.length > 0}
      <div class="p-3 bg-bg-base border border-border-default rounded-lg mb-3 text-text-secondary font-medium text-sm">
        Found {fastResults.length} conversations (sorted by relevance)
      </div>
      <div class="space-y-2">
        {#each fastResults as result (result.conversation_id)}
          <button
            class="block w-full text-left bg-bg-base border border-border-default rounded-lg p-3 cursor-pointer transition-colors duration-[--transition-default] text-inherit font-inherit hover:border-accent hover:bg-bg-surface"
            onclick={() => openConversation(result)}
          >
            <div class="flex justify-between items-start mb-2 gap-3">
              <div class="flex-1 flex flex-col gap-1.5">
                <span class="text-[15px] text-text-primary font-semibold leading-snug">
                  {result.title || `Conversation ${result.conversation_id.slice(0, 8)}`}
                </span>
                {#if result.project}
                  <span class="inline-block w-fit px-2 py-0.5 rounded-sm text-xs bg-bg-overlay text-syntax-property">
                    {result.project}
                  </span>
                {/if}
              </div>
              <span class="text-xs text-success-hover font-mono whitespace-nowrap">
                Score: {result.score.toFixed(2)}
              </span>
            </div>
            <div class="text-text-secondary leading-relaxed text-[13px]">
              {@html highlightMatch(result.snippet, $searchQuery)}
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>
