<script lang="ts">
  import { selectedConversation, viewMode as globalViewMode } from "$lib/stores";
  import ConversationView from "./ConversationView.svelte";
  import JsonView from "./JsonView.svelte";
  import SearchView from "./ConversationSearchView.svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { writeTextFile } from "@tauri-apps/plugin-fs";
  import { exportConversation } from "$lib/api";
  import type { ExportFormat } from "$lib/types";
  import { Tabs } from "bits-ui";
  import { DropdownMenu } from "bits-ui";
  import {
    ArrowLeft,
    FileDown,
    MessageSquare,
    Calendar,
    Activity,
    Braces,
    Search,
    BarChart3,
    Hash,
    Coins,
    DollarSign,
    Layers,
    Wrench,
    FileEdit,
    ChevronDown,
    FileJson,
    FileText,
    FileType,
    GraduationCap,
    Database,
    BookOpen,
  } from "lucide-svelte";

  let searchQuery = $state("");
  let conversation = $derived($selectedConversation);
  let exporting = $state(false);

  async function handleExport(format: ExportFormat) {
    if (!conversation || exporting) return;
    exporting = true;

    try {
      const content = await exportConversation(conversation.metadata.file_path, format);

      let extension = "txt";
      let filterName = "Text Files";
      if (format === "Json" || format === "JsonPretty") {
        extension = "json";
        filterName = "JSON Files";
      } else if (format === "Markdown") {
        extension = "md";
        filterName = "Markdown Files";
      } else if (format === "ChatML" || format === "Alpaca") {
        extension = "jsonl";
        filterName = "JSONL Files";
      } else if (format === "ShareGPT") {
        extension = "json";
        filterName = "JSON Files";
      }

      const filePath = await save({
        defaultPath: `${conversation.metadata.id}.${extension}`,
        filters: [{ name: filterName, extensions: [extension] }],
      });

      if (filePath) {
        await writeTextFile(filePath, content);
      }
    } catch (e) {
      console.error("Export failed:", e);
    } finally {
      exporting = false;
    }
  }
</script>

<div class="flex flex-col h-full bg-bg-base flex-1">
  {#if conversation}
    <div class="flex justify-between items-center px-5 py-3 border-b border-border-default bg-bg-surface gap-4">
      <button
        class="flex items-center gap-2 px-3 py-1.5 bg-transparent border border-border-strong rounded-md text-text-secondary text-[13px] cursor-pointer whitespace-nowrap transition-all duration-[--transition-default] hover:bg-bg-overlay hover:border-accent-hover hover:text-text-primary"
        onclick={() => globalViewMode.set("list")}
      >
        <ArrowLeft size={14} />
        Back
      </button>

      <div class="flex-1 min-w-0">
        <h2 class="m-0 mb-1 text-base font-semibold text-text-primary truncate">
          {conversation.metadata.title ||
            `Conversation ${conversation.metadata.id.slice(0, 8)}`}
        </h2>
        <div class="flex gap-2 text-[13px] text-text-tertiary">
          <span class="inline-flex items-center gap-1">
            <MessageSquare size={12} />
            {conversation.metadata.total_message_count} messages
          </span>
          <span>&bull;</span>
          <span class="inline-flex items-center gap-1">
            <Activity size={12} />
            {conversation.metadata.event_count} events
          </span>
          <span>&bull;</span>
          <span class="inline-flex items-center gap-1">
            <Calendar size={12} />
            {new Date(conversation.metadata.modified * 1000).toLocaleString()}
          </span>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <DropdownMenu.Root>
          <DropdownMenu.Trigger
            class="flex items-center gap-1.5 px-3 py-1.5 bg-success border-none rounded-md text-text-primary text-[13px] cursor-pointer transition-colors duration-[--transition-default] hover:bg-success-hover disabled:opacity-50 disabled:cursor-not-allowed"
            disabled={exporting}
          >
            <FileDown size={14} />
            {exporting ? "Exporting..." : "Export"}
            <ChevronDown size={12} />
          </DropdownMenu.Trigger>
          <DropdownMenu.Content
            class="min-w-[160px] mt-1 bg-bg-elevated border border-border-default rounded-md shadow-lg z-10 py-1"
          >
            <DropdownMenu.Item
              class="flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
              onSelect={() => handleExport("JsonPretty")}
            >
              <FileJson size={14} class="text-text-muted" />
              JSON (Pretty)
            </DropdownMenu.Item>
            <DropdownMenu.Item
              class="flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
              onSelect={() => handleExport("Json")}
            >
              <Braces size={14} class="text-text-muted" />
              JSON (Compact)
            </DropdownMenu.Item>
            <DropdownMenu.Item
              class="flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
              onSelect={() => handleExport("Markdown")}
            >
              <FileText size={14} class="text-text-muted" />
              Markdown
            </DropdownMenu.Item>
            <DropdownMenu.Item
              class="flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
              onSelect={() => handleExport("Text")}
            >
              <FileType size={14} class="text-text-muted" />
              Plain Text
            </DropdownMenu.Item>
            <DropdownMenu.Separator class="h-px bg-border-default my-1" />
            <div class="px-3 py-1 text-[11px] text-text-faint uppercase tracking-wider font-medium">
              LLM Training
            </div>
            <DropdownMenu.Item
              class="flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
              onSelect={() => handleExport("ChatML")}
            >
              <GraduationCap size={14} class="text-text-muted" />
              ChatML (OpenAI)
            </DropdownMenu.Item>
            <DropdownMenu.Item
              class="flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
              onSelect={() => handleExport("ShareGPT")}
            >
              <Database size={14} class="text-text-muted" />
              ShareGPT
            </DropdownMenu.Item>
            <DropdownMenu.Item
              class="flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
              onSelect={() => handleExport("Alpaca")}
            >
              <BookOpen size={14} class="text-text-muted" />
              Alpaca (Instruction)
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </div>
    </div>

    <Tabs.Root value="conversation" class="flex-1 flex flex-col overflow-hidden">
      <Tabs.List class="flex gap-1 bg-bg-base rounded-md p-1 mx-5 mt-2">
        <Tabs.Trigger
          value="conversation"
          class="flex items-center gap-1.5 px-3 py-1.5 border-none bg-transparent text-text-tertiary text-[13px] font-medium cursor-pointer rounded-sm transition-all duration-[--transition-default] hover:bg-bg-surface hover:text-text-secondary data-[state=active]:bg-accent-hover data-[state=active]:text-text-primary"
        >
          <MessageSquare size={14} />
          Conversation
        </Tabs.Trigger>
        <Tabs.Trigger
          value="json"
          class="flex items-center gap-1.5 px-3 py-1.5 border-none bg-transparent text-text-tertiary text-[13px] font-medium cursor-pointer rounded-sm transition-all duration-[--transition-default] hover:bg-bg-surface hover:text-text-secondary data-[state=active]:bg-accent-hover data-[state=active]:text-text-primary"
        >
          <Braces size={14} />
          Raw JSON
        </Tabs.Trigger>
        <Tabs.Trigger
          value="search"
          class="flex items-center gap-1.5 px-3 py-1.5 border-none bg-transparent text-text-tertiary text-[13px] font-medium cursor-pointer rounded-sm transition-all duration-[--transition-default] hover:bg-bg-surface hover:text-text-secondary data-[state=active]:bg-accent-hover data-[state=active]:text-text-primary"
        >
          <Search size={14} />
          Search
        </Tabs.Trigger>
        <Tabs.Trigger
          value="stats"
          class="flex items-center gap-1.5 px-3 py-1.5 border-none bg-transparent text-text-tertiary text-[13px] font-medium cursor-pointer rounded-sm transition-all duration-[--transition-default] hover:bg-bg-surface hover:text-text-secondary data-[state=active]:bg-accent-hover data-[state=active]:text-text-primary"
        >
          <BarChart3 size={14} />
          Stats
        </Tabs.Trigger>
      </Tabs.List>

      <div class="flex-1 overflow-hidden relative">
        <Tabs.Content value="conversation" class="h-full">
          <ConversationView {conversation} />
        </Tabs.Content>
        <Tabs.Content value="json" class="h-full">
          <JsonView data={conversation} />
        </Tabs.Content>
        <Tabs.Content value="search" class="h-full">
          <SearchView {conversation} bind:query={searchQuery} />
        </Tabs.Content>
        <Tabs.Content value="stats" class="h-full overflow-y-auto p-6">
          <div class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-4 mb-6">
            <div class="bg-bg-surface border border-border-default rounded-lg p-4">
              <div class="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wider mb-1">
                <MessageSquare size={12} />
                Messages
              </div>
              <div class="text-2xl font-bold text-text-primary tabular-nums">
                {conversation.metadata.total_message_count}
              </div>
              <div class="text-xs text-text-faint mt-1">
                {conversation.metadata.user_message_count} user / {conversation.metadata.total_message_count - conversation.metadata.user_message_count} assistant
              </div>
            </div>
            <div class="bg-bg-surface border border-border-default rounded-lg p-4">
              <div class="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wider mb-1">
                <Activity size={12} />
                Events
              </div>
              <div class="text-2xl font-bold text-text-primary tabular-nums">
                {conversation.metadata.event_count}
              </div>
            </div>
            <div class="bg-bg-surface border border-border-default rounded-lg p-4">
              <div class="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wider mb-1">
                <Hash size={12} />
                Input Tokens
              </div>
              <div class="text-2xl font-bold text-text-primary tabular-nums">
                {conversation.metadata.total_input_tokens.toLocaleString()}
              </div>
            </div>
            <div class="bg-bg-surface border border-border-default rounded-lg p-4">
              <div class="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wider mb-1">
                <Hash size={12} />
                Output Tokens
              </div>
              <div class="text-2xl font-bold text-text-primary tabular-nums">
                {conversation.metadata.total_output_tokens.toLocaleString()}
              </div>
            </div>
            <div class="bg-bg-surface border border-border-default rounded-lg p-4">
              <div class="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wider mb-1">
                <Coins size={12} />
                Total Tokens
              </div>
              <div class="text-2xl font-bold text-text-primary tabular-nums">
                {(conversation.metadata.total_input_tokens + conversation.metadata.total_output_tokens).toLocaleString()}
              </div>
            </div>
            <div class="bg-bg-surface border border-border-default rounded-lg p-4">
              <div class="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wider mb-1">
                <DollarSign size={12} />
                Estimated Cost
              </div>
              <div class="text-2xl font-bold text-text-primary tabular-nums">
                ${((conversation.metadata.total_input_tokens * 3 + conversation.metadata.total_output_tokens * 15) / 1_000_000).toFixed(4)}
              </div>
              <div class="text-xs text-text-faint mt-1">Based on Sonnet pricing ($3/$15 per MTok)</div>
            </div>
            <div class="bg-bg-surface border border-border-default rounded-lg p-4">
              <div class="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wider mb-1">
                <Layers size={12} />
                Sessions
              </div>
              <div class="text-2xl font-bold text-text-primary tabular-nums">
                {conversation.metadata.session_count}
              </div>
            </div>
            <div class="bg-bg-surface border border-border-default rounded-lg p-4">
              <div class="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wider mb-1">
                <Wrench size={12} />
                Tool Uses
              </div>
              <div class="text-2xl font-bold text-text-primary tabular-nums">
                {conversation.metadata.tool_use_count}
              </div>
            </div>
            <div class="bg-bg-surface border border-border-default rounded-lg p-4">
              <div class="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wider mb-1">
                <FileEdit size={12} />
                Files Modified
              </div>
              <div class="text-2xl font-bold text-text-primary tabular-nums">
                {conversation.metadata.artifact_count}
              </div>
            </div>
          </div>

          {#if conversation.metadata.tool_names.length > 0}
            <div class="mb-6">
              <h3 class="text-text-secondary text-sm m-0 mb-3 font-semibold">Tools Used</h3>
              <div class="flex flex-wrap gap-2">
                {#each conversation.metadata.tool_names as toolName}
                  <span class="bg-bg-elevated text-syntax-property px-2.5 py-1 rounded-sm text-[13px] font-mono">
                    {toolName}
                  </span>
                {/each}
              </div>
            </div>
          {/if}

          {#if conversation.metadata.artifacts.length > 0}
            <div class="mb-6">
              <h3 class="text-text-secondary text-sm m-0 mb-3 font-semibold">Files Modified</h3>
              <div class="flex flex-col gap-1">
                {#each conversation.metadata.artifacts as artifact}
                  <div class="font-mono text-[13px] text-success-hover px-2 py-1 bg-bg-sunken rounded-sm">
                    {artifact}
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        </Tabs.Content>
      </div>
    </Tabs.Root>
  {:else}
    <div class="flex flex-col items-center justify-center h-full text-text-tertiary text-center">
      <h3 class="m-0 mb-2 text-lg font-semibold text-text-secondary">No conversation selected</h3>
      <p class="m-0 text-sm">Select a conversation from the list to view details</p>
    </div>
  {/if}
</div>
