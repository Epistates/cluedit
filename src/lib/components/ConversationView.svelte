<script lang="ts">
  import type {
    Conversation,
    ConversationEvent,
    ContentBlockToolUse,
  } from "$lib/types";
  import {
    isChatMessage,
    isUserEvent,
    isAssistantEvent,
    extractText,
    getToolUses,
  } from "$lib/types";
  import { marked } from "marked";
  import { highlightCode } from "$lib/highlight";
  import { Collapsible } from "bits-ui";
  import {
    User,
    Bot,
    Terminal,
    FileEdit,
    Search,
    Eye,
    FolderOpen,
    Globe,
    Wrench,
    ChevronRight,
    ChevronDown,
  } from "lucide-svelte";
  import Skeleton from "./Skeleton.svelte";

  let { conversation }: { conversation: Conversation } = $props();

  let openToolCalls = $state(new Set<string>());

  // Map tool names to icons
  const toolIcons: Record<string, typeof Terminal> = {
    Bash: Terminal,
    Write: FileEdit,
    Edit: FileEdit,
    Read: Eye,
    Grep: Search,
    Glob: FolderOpen,
    WebSearch: Globe,
    WebFetch: Globe,
  };

  function getToolIcon(name: string): typeof Terminal {
    return toolIcons[name] || Wrench;
  }

  // Configure marked with async Shiki code block rendering
  const renderer = new marked.Renderer();

  // Track pending code blocks for async Shiki highlighting
  let codeBlockCounter = 0;
  const codeBlockPlaceholders = new Map<string, { code: string; lang: string }>();

  renderer.code = function ({ text, lang }: { text: string; lang?: string | undefined }) {
    const id = `shiki-${codeBlockCounter++}`;
    const language = lang || "text";
    codeBlockPlaceholders.set(id, { code: text, lang: language });

    // Return placeholder; will be replaced after Shiki highlights
    const escaped = text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
    return `<pre class="shiki" data-shiki-id="${id}"><code>${escaped}</code></pre>`;
  };

  marked.setOptions({
    breaks: true,
    gfm: true,
    renderer,
  });

  function getMessageContent(event: ConversationEvent): string {
    if (isUserEvent(event) || isAssistantEvent(event)) {
      return extractText(event.message.content);
    }
    return "";
  }

  function getToolCalls(event: ConversationEvent): ContentBlockToolUse[] {
    if (isAssistantEvent(event)) {
      return getToolUses(event.message.content);
    }
    return [];
  }

  function formatTimestamp(event: ConversationEvent): string {
    if (isUserEvent(event) || isAssistantEvent(event)) {
      const ts = event.timestamp;
      if (ts) {
        return new Date(ts).toLocaleTimeString();
      }
    }
    return "";
  }

  function renderMarkdown(text: string): string {
    return marked.parse(text, { async: false }) as string;
  }

  // Post-render: replace placeholders with Shiki output
  let containerEl: HTMLElement | undefined = $state(undefined);

  async function applyShikiHighlighting() {
    if (!containerEl || codeBlockPlaceholders.size === 0) return;

    for (const [id, { code, lang }] of codeBlockPlaceholders) {
      const el = containerEl.querySelector(`[data-shiki-id="${id}"]`);
      if (el) {
        try {
          const html = await highlightCode(code, lang);
          el.outerHTML = html;
        } catch {
          // Keep fallback
        }
      }
    }
    codeBlockPlaceholders.clear();
  }

  let messages = $derived(
    conversation.events.filter(
      (e) => isChatMessage(e) && getMessageContent(e).trim().length > 0
    )
  );

  $effect(() => {
    // Trigger Shiki highlighting after messages render
    if (messages && containerEl) {
      // Small delay to wait for DOM update
      requestAnimationFrame(() => applyShikiHighlighting());
    }
  });
</script>

<div class="h-full overflow-y-auto bg-bg-base" bind:this={containerEl}>
  <div class="p-6 max-w-[900px] mx-auto">
    {#each messages as event, index (index)}
      {@const isUser = isUserEvent(event)}
      {@const content = getMessageContent(event)}
      {@const toolCalls = getToolCalls(event)}
      {@const timestamp = formatTimestamp(event)}

      <div
        class="mb-6 p-4 rounded-lg border-l-[3px]"
        class:bg-msg-user-bg={isUser}
        class:border-l-msg-user-border={isUser}
        class:bg-msg-assistant-bg={!isUser}
        class:border-l-msg-assistant-border={!isUser}
      >
        <div class="flex justify-between items-center mb-3">
          <span
            class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-semibold uppercase tracking-wider text-text-primary"
            class:bg-success={isUser}
            class:bg-accent-hover={!isUser}
          >
            {#if isUser}
              <User size={12} />
              User
            {:else}
              <Bot size={12} />
              Assistant
            {/if}
          </span>
          {#if timestamp}
            <span class="text-xs text-text-muted">{timestamp}</span>
          {/if}
        </div>

        {#if content}
          <div class="prose">
            {@html renderMarkdown(content)}
          </div>
        {/if}

        {#if toolCalls.length > 0}
          <div class="mt-3 flex flex-col gap-2">
            {#each toolCalls as tool, toolIdx}
              {@const IconComponent = getToolIcon(tool.name)}
              {@const toolKey = `${index}-${toolIdx}`}
              <Collapsible.Root
                open={openToolCalls.has(toolKey)}
                onOpenChange={(open) => {
                  const next = new Set(openToolCalls);
                  if (open) next.add(toolKey);
                  else next.delete(toolKey);
                  openToolCalls = next;
                }}
                class="bg-bg-sunken border border-border-default rounded-md overflow-hidden"
              >
                <Collapsible.Trigger class="flex items-center gap-2 w-full px-3 py-2 bg-transparent border-none text-left cursor-pointer text-inherit font-inherit hover:bg-bg-base transition-colors duration-[--transition-fast]">
                  {#if openToolCalls.has(toolKey)}
                    <ChevronDown size={14} class="text-text-muted shrink-0" />
                  {:else}
                    <ChevronRight size={14} class="text-text-muted shrink-0" />
                  {/if}
                  <IconComponent size={14} class="text-syntax-property shrink-0" />
                  <span class="font-semibold text-syntax-property text-[13px]">{tool.name}</span>
                </Collapsible.Trigger>
                <Collapsible.Content>
                  {#if tool.input}
                    <pre class="m-0 px-3 py-2 bg-bg-deep rounded-none text-xs text-text-muted overflow-x-auto font-mono leading-relaxed border-t border-border-default">{JSON.stringify(tool.input, null, 2)}</pre>
                  {/if}
                </Collapsible.Content>
              </Collapsible.Root>
            {/each}
          </div>
        {/if}
      </div>
    {/each}

    {#if messages.length === 0}
      <div class="text-center py-16 text-text-muted">
        <p>No messages in this conversation</p>
      </div>
    {/if}
  </div>
</div>
