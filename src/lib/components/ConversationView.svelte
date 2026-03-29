<script lang="ts">
  import type {
    Conversation,
    ConversationEvent,
    ContentBlockToolUse,
    ContentBlockToolResult,
  } from "$lib/types";
  import {
    isChatMessage,
    isUserEvent,
    isAssistantEvent,
    isToolResultOnly,
    extractText,
    extractToolResultText,
    getToolUses,
    getToolResults,
  } from "$lib/types";
  import { marked } from "marked";
  import DOMPurify from "dompurify";
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
    GitBranch,
  } from "lucide-svelte";
  import Skeleton from "./Skeleton.svelte";

  let {
    conversation,
    onbranch,
    branchingEventIndex = null,
  }: {
    conversation: Conversation;
    onbranch?: (e: CustomEvent<{ eventIndex: number }>) => void;
    branchingEventIndex?: number | null;
  } = $props();

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

  /** Get a compact summary for the tool call header line */
  function getToolSummary(tool: ContentBlockToolUse): string {
    const input = tool.input as Record<string, unknown>;
    switch (tool.name) {
      case "Bash":
        return (input.command as string || "").slice(0, 80);
      case "Read":
        return (input.file_path as string || "").split("/").slice(-2).join("/");
      case "Write":
        return (input.file_path as string || "").split("/").slice(-2).join("/");
      case "Edit":
        return (input.file_path as string || "").split("/").slice(-2).join("/");
      case "Grep":
        return `/${input.pattern || ""}/ ${(input.path as string || "").split("/").pop() || ""}`;
      case "Glob":
        return (input.pattern as string || "");
      case "Agent":
        return (input.description as string || "").slice(0, 60);
      default:
        return "";
    }
  }

  function getToolCalls(event: ConversationEvent): ContentBlockToolUse[] {
    if (isAssistantEvent(event)) {
      return getToolUses(event.message.content);
    }
    return [];
  }

  /** Build a lookup map of tool_use_id → tool_result for the entire conversation */
  let toolResultMap = $derived.by(() => {
    const map = new Map<string, ContentBlockToolResult>();
    for (const evt of conversation.events) {
      if (!isUserEvent(evt)) continue;
      const content = evt.message.content;
      if (typeof content === "string") continue;
      for (const block of content) {
        if (block.type === "tool_result") {
          const tr = block as ContentBlockToolResult;
          map.set(tr.tool_use_id, tr);
        }
      }
    }
    return map;
  });

  function findToolResult(_eventIndex: number, toolUseId: string): ContentBlockToolResult | null {
    return toolResultMap.get(toolUseId) ?? null;
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
    return DOMPurify.sanitize(marked.parse(text, { async: false }) as string);
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

  // Track both the event and its index in the full events array
  let messages = $derived(
    conversation.events
      .map((e, idx) => ({ event: e, eventIndex: idx }))
      .filter(
        ({ event }) =>
          isChatMessage(event) &&
          (getMessageContent(event).trim().length > 0 || getToolCalls(event).length > 0)
      )
  );

  function handleBranchFromHere(eventIndex: number) {
    if (!onbranch || branchingEventIndex !== null) return;
    onbranch(new CustomEvent("branch", { detail: { eventIndex } }));
  }

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
    {#each messages as { event, eventIndex }, index (index)}
      {@const isUser = isUserEvent(event)}
      {@const content = getMessageContent(event)}
      {@const toolCalls = getToolCalls(event)}
      {@const timestamp = formatTimestamp(event)}

      <div
        class="group/msg mb-6 p-4 rounded-lg border-l-[3px] relative"
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
          <div class="flex items-center gap-2">
            {#if onbranch}
              <button
                class="flex items-center gap-1 px-1.5 py-0.5 bg-transparent border border-transparent rounded text-[10px] text-text-faint cursor-pointer transition-all opacity-0 group-hover/msg:opacity-100 hover:!border-accent-hover hover:!text-accent-hover hover:!bg-accent-hover/10 disabled:opacity-50 disabled:cursor-not-allowed"
                onclick={() => handleBranchFromHere(eventIndex)}
                disabled={branchingEventIndex !== null}
                title="Branch conversation from this point (creates a duplicate with new IDs up to this message)"
              >
                <GitBranch size={10} />
                {branchingEventIndex === eventIndex ? "branching..." : "branch here"}
              </button>
            {/if}
            {#if timestamp}
              <span class="text-xs text-text-muted">{timestamp}</span>
            {/if}
          </div>
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
              {@const toolResult = findToolResult(eventIndex, tool.id)}
              {@const resultText = toolResult ? extractToolResultText(toolResult) : ""}
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
                {@const toolSummary = getToolSummary(tool)}
                <Collapsible.Trigger class="flex items-center gap-2 w-full px-3 py-2 bg-transparent border-none text-left cursor-pointer text-inherit font-inherit hover:bg-bg-base transition-colors duration-[--transition-fast]">
                  {#if openToolCalls.has(toolKey)}
                    <ChevronDown size={14} class="text-text-muted shrink-0" />
                  {:else}
                    <ChevronRight size={14} class="text-text-muted shrink-0" />
                  {/if}
                  <IconComponent size={14} class="text-syntax-property shrink-0" />
                  <span class="font-semibold text-syntax-property text-[13px]">{tool.name}</span>
                  {#if toolSummary}
                    <span class="text-[12px] text-text-muted font-mono truncate">{toolSummary}</span>
                  {/if}
                  {#if toolResult?.is_error}
                    <span class="text-[10px] text-danger font-medium ml-auto shrink-0">ERROR</span>
                  {/if}
                </Collapsible.Trigger>

                <!-- Always show compact result preview when collapsed -->
                {#if !openToolCalls.has(toolKey) && resultText}
                  <div class="px-3 py-1.5 border-t border-border-default">
                    <pre class="m-0 text-[11px] font-mono leading-relaxed overflow-hidden max-h-[60px] {toolResult?.is_error ? 'text-danger/70' : 'text-text-faint'}">{resultText.slice(0, 300)}{resultText.length > 300 ? "..." : ""}</pre>
                  </div>
                {/if}

                <Collapsible.Content>
                  {#if tool.input}
                    <div class="px-3 py-1.5 text-[10px] text-text-faint uppercase tracking-wider font-medium border-t border-border-default">Input</div>
                    <pre class="m-0 px-3 py-2 bg-bg-deep rounded-none text-xs text-text-muted overflow-x-auto font-mono leading-relaxed max-h-[200px]">{JSON.stringify(tool.input, null, 2)}</pre>
                  {/if}
                  {#if resultText}
                    <div class="px-3 py-1.5 text-[10px] uppercase tracking-wider font-medium border-t border-border-default {toolResult?.is_error ? 'text-danger' : 'text-text-faint'}">
                      {toolResult?.is_error ? "Error" : "Output"}
                    </div>
                    <pre class="m-0 px-3 py-2 bg-bg-deep rounded-none text-xs overflow-x-auto font-mono leading-relaxed max-h-[300px] overflow-y-auto {toolResult?.is_error ? 'text-danger' : 'text-text-secondary'}">{resultText}</pre>
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
