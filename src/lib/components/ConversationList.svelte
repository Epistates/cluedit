<script lang="ts">
  import {
    filteredConversations,
    selectedConversation,
    viewMode,
  } from "$lib/stores";
  import {
    setLoading,
    setError,
    clearStatus,
    setMessage,
  } from "$lib/stores/statusStore";
  import { readConversation, getConversationMetadata, findParentConversation } from "$lib/api";
  import type { ConversationMetadata } from "$lib/types";
  import { onMount } from "svelte";
  import {
    MessageSquare,
    FileText,
    HardDrive,
    Link,
    Package,
  } from "lucide-svelte";
  import Skeleton from "./Skeleton.svelte";

  let enrichedConversations: Record<string, ConversationMetadata> = $state({});
  let isEnriching = $state(false);
  let lastConversationsLength = $state(0);

  async function enrichVisibleConversations() {
    if (isEnriching) return;

    const visible = $filteredConversations.slice(0, 10);
    if (visible.length === 0) {
      clearStatus();
      return;
    }

    const toEnrich = visible.filter(
      (conv) => !enrichedConversations[conv.id]
    );

    if (toEnrich.length === 0) {
      clearStatus();
      return;
    }

    isEnriching = true;
    setLoading(`Loading conversation details...`, {
      current: 0,
      total: toEnrich.length,
    });

    let enrichedCount = 0;

    for (const conv of toEnrich) {
      try {
        enrichedCount++;
        setLoading(`Analyzing conversations...`, {
          current: enrichedCount,
          total: toEnrich.length,
        });

        const fullMetadata = await getConversationMetadata(conv.file_path);

        enrichedConversations = {
          ...enrichedConversations,
          [conv.id]: fullMetadata,
        };
      } catch (e) {
        console.error(`Failed to enrich ${conv.id}:`, e);
        setError(`Failed to load conversation details: ${e}`);
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
    }

    isEnriching = false;
    setMessage(`Loaded ${enrichedCount} conversation details`);
  }

  onMount(() => {
    lastConversationsLength = $filteredConversations.length;
    enrichVisibleConversations();
  });

  $effect(() => {
    const len = $filteredConversations.length;
    if (len > 0 && len !== lastConversationsLength) {
      lastConversationsLength = len;
      enrichedConversations = {};
      enrichVisibleConversations();
    }
  });

  function getMetadata(conv: ConversationMetadata): ConversationMetadata {
    return enrichedConversations[conv.id] || conv;
  }

  async function openConversation(metadata: ConversationMetadata) {
    try {
      setLoading("Opening conversation...");
      const conversation = await readConversation(metadata.file_path);
      selectedConversation.set(conversation);
      viewMode.set("detail");
      setMessage("Conversation loaded successfully");
    } catch (e) {
      console.error("Failed to load conversation:", e);
      setError(`Failed to load conversation: ${e}`);
    }
  }

  async function openParentConversation(event: Event, metadata: ConversationMetadata) {
    event.stopPropagation();
    if (!metadata.continued_from_id) return;

    try {
      setLoading("Finding parent conversation...");
      const parentPath = await findParentConversation(metadata.continued_from_id);
      if (parentPath) {
        const conversation = await readConversation(parentPath);
        selectedConversation.set(conversation);
        viewMode.set("detail");
        setMessage("Parent conversation loaded");
      } else {
        setMessage("Parent conversation not found");
      }
    } catch (e) {
      setError(`Failed to find parent: ${e}`);
    }
  }

  function formatDate(timestamp: number): string {
    return new Date(timestamp * 1000).toLocaleString();
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function truncate(text: string | null, maxLength: number): string {
    if (!text) return "";
    return text.length > maxLength ? text.slice(0, maxLength) + "..." : text;
  }
</script>

<div class="flex-1 flex flex-col h-full overflow-hidden">
  <div class="px-6 py-4 border-b border-border-default bg-bg-base">
    <h2 class="m-0 text-xl font-semibold text-text-primary">
      Conversations ({$filteredConversations.length})
    </h2>
  </div>

  {#if $filteredConversations.length === 0}
    <div class="p-12 text-center text-text-muted text-base">
      No conversations found
    </div>
  {:else}
    <div class="flex-1 overflow-y-auto p-3 bg-bg-surface space-y-2">
      {#each $filteredConversations as conversation (conversation.id)}
        {@const metadata = getMetadata(conversation)}
        <button
          class="block w-full text-left bg-bg-base border border-border-default rounded-lg p-3 cursor-pointer transition-colors duration-[--transition-default] text-inherit font-inherit hover:border-accent hover:bg-bg-surface"
          onclick={() => openConversation(conversation)}
        >
          <div class="flex justify-between items-start mb-2 gap-3">
            <div class="flex-1 flex flex-col gap-1.5">
              <span class="text-[15px] text-text-primary font-semibold leading-snug">
                {metadata.title ||
                  truncate(metadata.first_message, 80) ||
                  `Conversation ${conversation.id.slice(0, 8)}`}
              </span>
              <div class="flex gap-1.5 flex-wrap">
                {#if metadata.is_continuation}
                  <span
                    class="inline-flex items-center gap-1 px-2 py-0.5 rounded-sm text-xs font-medium bg-bg-overlay text-syntax-property cursor-pointer hover:bg-bg-muted hover:text-text-primary transition-colors duration-[--transition-fast]"
                    role="button"
                    tabindex="0"
                    onclick={(e) => openParentConversation(e, metadata)}
                    onkeydown={(e) => e.key === "Enter" && openParentConversation(e, metadata)}
                    title="Click to open parent conversation"
                  >
                    <Link size={11} />
                    Continued
                  </span>
                {/if}
                {#if metadata.has_compaction}
                  <span class="inline-block px-2 py-0.5 rounded-sm text-xs font-medium bg-bg-overlay text-syntax-string">
                    Compacted
                  </span>
                {/if}
                {#if metadata.artifact_count > 0}
                  <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-sm text-xs font-medium bg-bg-overlay text-success-hover">
                    <FileText size={11} />
                    {metadata.artifact_count} files
                  </span>
                {/if}
              </div>
            </div>
            <span class="text-xs text-text-muted whitespace-nowrap">
              {formatDate(metadata.modified)}
            </span>
          </div>

          <div class="mb-2">
            {#if metadata.summary}
              <p class="m-0 text-text-secondary leading-relaxed text-sm font-medium">
                {metadata.summary}
              </p>
            {:else if metadata.last_user_message}
              <p class="m-0 text-text-tertiary leading-normal text-[13px]">
                {truncate(metadata.last_user_message, 200)}
              </p>
            {:else}
              <p class="m-0 text-text-faint italic text-sm">
                No preview available
              </p>
            {/if}
          </div>

          <div class="flex gap-3 text-xs text-text-muted items-center flex-wrap">
            <span class="inline-flex items-center gap-1">
              <MessageSquare size={12} />
              {metadata.total_message_count} messages
            </span>
            <span class="inline-flex items-center gap-1">
              <Package size={12} />
              {conversation.event_count} events
            </span>
            <span class="inline-flex items-center gap-1">
              <HardDrive size={12} />
              {formatSize(conversation.size_bytes)}
            </span>
            {#if metadata.topics.length > 0}
              <div class="flex gap-1 ml-auto">
                {#each metadata.topics.slice(0, 3) as topic}
                  <span class="bg-bg-elevated text-syntax-keyword px-2 py-0.5 rounded-sm text-[11px] font-mono">
                    {topic}
                  </span>
                {/each}
              </div>
            {/if}
          </div>
        </button>
      {/each}
    </div>
  {/if}
</div>
