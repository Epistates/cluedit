<script lang="ts">
  import type { Conversation, ConversationEvent } from "$lib/types";
  import {
    isUserEvent,
    isAssistantEvent,
    isChatMessage,
    extractText,
  } from "$lib/types";
  import { Switch } from "bits-ui";
  import { Search, User, Bot } from "lucide-svelte";

  let { conversation, query = $bindable("") }: { conversation: Conversation; query: string } = $props();

  interface SearchResult {
    eventIndex: number;
    event: ConversationEvent;
    matches: { text: string; highlighted: string }[];
  }

  let caseSensitive = $state(false);
  let useRegex = $state(false);

  function escapeHtml(text: string): string {
    return text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function escapeRegexStr(str: string): string {
    return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }

  function highlightMatch(text: string, q: string): string {
    if (!q) return escapeHtml(text);
    const escaped = escapeHtml(text);
    const escapedQuery = escapeHtml(q);

    try {
      const flags = caseSensitive ? "g" : "gi";
      const pattern = useRegex ? escapedQuery : escapeRegexStr(escapedQuery);
      const regex = new RegExp(`(${pattern})`, flags);
      return escaped.replace(regex, "<mark>$1</mark>");
    } catch {
      return escaped;
    }
  }

  function searchConversation(
    events: ConversationEvent[],
    q: string
  ): SearchResult[] {
    if (!q.trim()) return [];

    const results: SearchResult[] = [];

    events.forEach((event, index) => {
      if (!isChatMessage(event)) return;
      if (!isUserEvent(event) && !isAssistantEvent(event)) return;

      const content = extractText(event.message.content);
      if (!content) return;

      const matches: { text: string; highlighted: string }[] = [];

      const searchText = caseSensitive ? content : content.toLowerCase();
      const searchQ = caseSensitive ? q : q.toLowerCase();

      const isMatch = useRegex
        ? (() => {
            try {
              return new RegExp(q, caseSensitive ? "" : "i").test(content);
            } catch {
              return false;
            }
          })()
        : searchText.includes(searchQ);

      if (isMatch) {
        const contextLength = 100;
        let regex: RegExp;
        try {
          regex = useRegex
            ? new RegExp(q, "gi")
            : new RegExp(escapeRegexStr(q), caseSensitive ? "g" : "gi");
        } catch {
          return;
        }

        let match;
        while ((match = regex.exec(content)) !== null) {
          const start = Math.max(0, match.index - contextLength);
          const end = Math.min(
            content.length,
            match.index + match[0].length + contextLength
          );
          const snippet = content.substring(start, end);
          const prefix = start > 0 ? "..." : "";
          const suffix = end < content.length ? "..." : "";
          const fullSnippet = prefix + snippet + suffix;

          matches.push({
            text: fullSnippet,
            highlighted: highlightMatch(fullSnippet, q),
          });
        }
      }

      if (matches.length > 0) {
        results.push({ eventIndex: index, event, matches });
      }
    });

    return results;
  }

  let results = $derived(searchConversation(conversation.events, query));
</script>

<div class="flex flex-col h-full bg-bg-base">
  <div class="p-4 border-b border-border-default bg-bg-surface">
    <div class="mb-3">
      <div class="relative">
        <Search
          size={16}
          class="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted pointer-events-none"
        />
        <input
          type="text"
          bind:value={query}
          placeholder="Search in conversation..."
          class="w-full bg-bg-overlay border border-border-strong pl-10 pr-4 py-3 rounded-md text-text-secondary text-sm font-inherit focus:outline-none focus:border-accent-hover"
          autofocus
        />
      </div>
    </div>
    <div class="flex items-center gap-6">
      <label class="flex items-center gap-2 text-[13px] text-text-secondary cursor-pointer">
        <Switch.Root
          bind:checked={caseSensitive}
          class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out data-[state=checked]:bg-accent data-[state=unchecked]:bg-bg-overlay"
        >
          <Switch.Thumb
            class="pointer-events-none inline-block h-4 w-4 rounded-full bg-text-primary shadow-sm transition-transform duration-200 ease-in-out translate-x-0 data-[state=checked]:translate-x-4"
          />
        </Switch.Root>
        Case sensitive
      </label>
      <label class="flex items-center gap-2 text-[13px] text-text-secondary cursor-pointer">
        <Switch.Root
          bind:checked={useRegex}
          class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out data-[state=checked]:bg-accent data-[state=unchecked]:bg-bg-overlay"
        >
          <Switch.Thumb
            class="pointer-events-none inline-block h-4 w-4 rounded-full bg-text-primary shadow-sm transition-transform duration-200 ease-in-out translate-x-0 data-[state=checked]:translate-x-4"
          />
        </Switch.Root>
        Regex
      </label>
    </div>
    {#if query}
      <div class="text-xs text-text-tertiary mt-2">
        {results.length} result{results.length !== 1 ? "s" : ""} found
      </div>
    {/if}
  </div>

  <div class="flex-1 overflow-y-auto p-4">
    {#if query && results.length > 0}
      <div class="space-y-4">
        {#each results as result}
          <div class="p-3 bg-bg-surface rounded-md border-l-[3px] border-l-accent-hover">
            <div class="flex justify-between items-center mb-2">
              <span
                class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-semibold uppercase tracking-wider text-text-primary"
                class:bg-success={isUserEvent(result.event)}
                class:bg-accent-hover={!isUserEvent(result.event)}
              >
                {#if isUserEvent(result.event)}
                  <User size={10} />
                  User
                {:else}
                  <Bot size={10} />
                  Assistant
                {/if}
              </span>
              <span class="text-xs text-text-muted">Event #{result.eventIndex + 1}</span>
            </div>
            <div class="flex flex-col gap-2">
              {#each result.matches as match}
                <div class="p-2 bg-bg-sunken rounded-sm text-[13px] leading-normal text-text-secondary">
                  {@html match.highlighted}
                </div>
              {/each}
            </div>
          </div>
        {/each}
      </div>
    {:else if query}
      <div class="text-center py-16 text-text-muted">
        <p>No matches found for "<strong class="text-text-secondary">{query}</strong>"</p>
        <p class="text-sm text-text-faint mt-2">Try different search terms or disable regex mode</p>
      </div>
    {:else}
      <div class="text-center py-16 text-text-muted">
        <p>Enter search terms to find messages in this conversation</p>
        <div class="mt-6 text-left max-w-[400px] mx-auto">
          <h4 class="text-text-secondary mb-3 text-sm">Search Tips:</h4>
          <ul class="list-none p-0 space-y-1.5">
            <li class="text-[13px] pl-5 relative before:content-['•'] before:absolute before:left-0 before:text-accent">
              Use plain text for simple searches
            </li>
            <li class="text-[13px] pl-5 relative before:content-['•'] before:absolute before:left-0 before:text-accent">
              Enable "Regex" for advanced pattern matching
            </li>
            <li class="text-[13px] pl-5 relative before:content-['•'] before:absolute before:left-0 before:text-accent">
              Toggle "Case sensitive" to match exact casing
            </li>
          </ul>
        </div>
      </div>
    {/if}
  </div>
</div>
