<script lang="ts">
  import { selectedConversation, viewMode as globalViewMode, conversations } from "$lib/stores";
  import ConversationView from "./ConversationView.svelte";
  import JsonView from "./JsonView.svelte";
  import SearchView from "./ConversationSearchView.svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import {
    exportConversationToFile,
    createBackup,
    listBackups,
    restoreBackup,
    branchConversation,
    branchFromBackup,
    deleteBackup,
    readConversation,
    listConversations,
  } from "$lib/api";
  import type { ExportFormat, BackupInfo } from "$lib/types";
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
    Archive,
    Plus,
    RotateCcw,
    Trash2,
    GitBranch,
    Clock,
    Shield,
    Copy,
    Scissors,
    Loader2,
    Upload,
  } from "lucide-svelte";
  import PublishDialog from "./PublishDialog.svelte";

  let searchQuery = $state("");
  let conversation = $derived($selectedConversation);
  let exporting = $state(false);

  // Backup state
  let backups = $state<BackupInfo[]>([]);
  let backupLabel = $state("");
  let creatingBackup = $state(false);
  let restoringId = $state<string | null>(null);
  let branchingFull = $state(false);
  let branchingBackupId = $state<string | null>(null);
  let branchingEventIndex = $state<number | null>(null);
  let deletingId = $state<string | null>(null);
  let backupError = $state<string | null>(null);
  let backupSuccess = $state<string | null>(null);

  /// Derive the project directory path from a conversation's file_path.
  /// Cross-platform: strip the final path component (handles both / and \).
  function getProjectPath(filePath: string): string {
    const lastSlash = Math.max(filePath.lastIndexOf("/"), filePath.lastIndexOf("\\"));
    return lastSlash > 0 ? filePath.substring(0, lastSlash) : filePath;
  }

  async function refreshConversationList() {
    const conv = $selectedConversation;
    if (!conv?.metadata.project) return;
    try {
      const projectPath = getProjectPath(conv.metadata.file_path);
      const updated = await listConversations(projectPath);
      conversations.set(updated);
    } catch (e) {
      console.error("Failed to refresh conversation list:", e);
    }
  }

  async function loadBackups() {
    if (!conversation) return;
    try {
      backups = await listBackups(conversation.metadata.id);
      backupError = null;
    } catch (e) {
      backupError = `Failed to load backups: ${e}`;
      console.error("Failed to load backups:", e);
    }
  }

  async function handleCreateBackup() {
    if (!conversation || creatingBackup) return;
    const label = backupLabel.trim() || `Backup ${new Date().toLocaleString()}`;
    creatingBackup = true;
    backupError = null;
    backupSuccess = null;

    try {
      const info = await createBackup(conversation.metadata.file_path, label);
      backupSuccess = `Backup created: ${info.event_count} events, ${formatBytes(info.size_bytes)}`;
      backupLabel = "";
      await loadBackups();
    } catch (e) {
      backupError = `Backup failed: ${e}`;
    } finally {
      creatingBackup = false;
    }
  }

  async function handleRestore(backup: BackupInfo) {
    if (restoringId) return;
    restoringId = backup.id;
    backupError = null;
    backupSuccess = null;

    // Capture file_path before any reactive updates
    const filePath = conversation!.metadata.file_path;

    try {
      const safetyBackup = await restoreBackup(backup.id);
      const safetyMsg = safetyBackup.id
        ? `Safety backup created: ${safetyBackup.id.slice(0, 8)}`
        : safetyBackup.label;
      backupSuccess = `Restored to "${backup.label}". ${safetyMsg}`;
      await loadBackups();
      // Reload the conversation to reflect restored state
      const refreshed = await readConversation(filePath);
      selectedConversation.set(refreshed);
      await refreshConversationList();
    } catch (e) {
      backupError = `Restore failed: ${e}`;
    } finally {
      restoringId = null;
    }
  }

  async function handleBranchFull() {
    if (!conversation || branchingFull) return;
    branchingFull = true;
    backupError = null;
    backupSuccess = null;

    try {
      const result = await branchConversation(conversation.metadata.file_path);
      backupSuccess = `Branched: ${result.new_conversation_id.slice(0, 8)}... (${result.event_count} events, ${result.ids_remapped} IDs remapped)`;
      await refreshConversationList();
    } catch (e) {
      backupError = `Branch failed: ${e}`;
    } finally {
      branchingFull = false;
    }
  }

  async function handleBranchFromBackup(backup: BackupInfo) {
    if (branchingBackupId) return;
    branchingBackupId = backup.id;
    backupError = null;
    backupSuccess = null;

    try {
      const result = await branchFromBackup(backup.id);
      backupSuccess = `Branched from backup: ${result.new_conversation_id.slice(0, 8)}... (${result.event_count} events, ${result.ids_remapped} IDs remapped)`;
      await refreshConversationList();
    } catch (e) {
      backupError = `Branch from backup failed: ${e}`;
    } finally {
      branchingBackupId = null;
    }
  }

  async function handleDelete(backup: BackupInfo) {
    if (deletingId) return;
    deletingId = backup.id;
    backupError = null;
    backupSuccess = null;

    try {
      await deleteBackup(backup.id);
      backupSuccess = `Backup deleted`;
      await loadBackups();
    } catch (e) {
      backupError = `Delete failed: ${e}`;
    } finally {
      deletingId = null;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function formatBackupDate(isoDate: string): string {
    return new Date(isoDate).toLocaleString();
  }

  // Handle branch-from-here events dispatched by ConversationView.
  // Returns a promise so the child can track completion.
  async function handleBranchFromEvent(e: CustomEvent<{ eventIndex: number }>) {
    if (!conversation) return;
    branchingEventIndex = e.detail.eventIndex;
    backupError = null;
    backupSuccess = null;

    try {
      const result = await branchConversation(
        conversation.metadata.file_path,
        e.detail.eventIndex
      );
      backupSuccess = `Branched at event ${e.detail.eventIndex}: ${result.new_conversation_id.slice(0, 8)}... (${result.event_count} events)`;
      await refreshConversationList();
    } catch (err) {
      backupError = `Branch failed: ${err}`;
    } finally {
      branchingEventIndex = null;
    }
  }

  let exportSuccess = $state<string | null>(null);
  let publishOpen = $state(false);

  function cleanProjectName(raw: string): string {
    const segments = raw.split("-").filter(Boolean);
    if (segments.length === 0) return raw;
    const last = segments[segments.length - 1];
    const generic = ["work", "src", "dev", "home", "Users", "tmp", "var"];
    if (segments.length >= 2 && generic.includes(last)) {
      return segments.slice(-2).join("-");
    }
    return last;
  }

  async function handleExport(format: ExportFormat) {
    if (!conversation || exporting) return;
    exporting = true;
    exportSuccess = null;

    try {
      let extension = "txt";
      let filterName = "Text Files";
      if (format === "Json" || format === "JsonPretty") {
        extension = "json";
        filterName = "JSON Files";
      } else if (format === "Markdown") {
        extension = "md";
        filterName = "Markdown Files";
      } else if (format === "ChatML" || format === "ChatMLTools" || format === "Alpaca") {
        extension = "jsonl";
        filterName = "JSONL Files";
      } else if (format === "ShareGPT") {
        extension = "json";
        filterName = "JSON Files";
      }

      // project_name-conversation_id
      const project = conversation.metadata.project
        ? cleanProjectName(conversation.metadata.project)
        : "export";
      const id = conversation.metadata.id.slice(0, 8);
      const baseName = `${project}-${id}`;

      const outputPath = await save({
        defaultPath: `${baseName}.${extension}`,
        filters: [{ name: filterName, extensions: [extension] }],
      });

      if (outputPath) {
        await exportConversationToFile(conversation.metadata.file_path, format, outputPath);
        exportSuccess = `Exported as ${format}`;
        setTimeout(() => { exportSuccess = null; }, 4000);
      }
    } catch (e) {
      exportSuccess = `Export failed: ${e}`;
      setTimeout(() => { exportSuccess = null; }, 5000);
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
            {#if exporting}
              <Loader2 size={14} class="animate-spin" />
              Exporting...
            {:else}
              <FileDown size={14} />
              Export
            {/if}
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
              ChatML (text only)
            </DropdownMenu.Item>
            <DropdownMenu.Item
              class="flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
              onSelect={() => handleExport("ChatMLTools")}
            >
              <Wrench size={14} class="text-text-muted" />
              ChatML + Tools (agentic)
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
            <DropdownMenu.Separator class="h-px bg-border-default my-1" />
            <DropdownMenu.Item
              class="flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
              onSelect={() => { publishOpen = true; }}
            >
              <Upload size={14} class="text-accent-hover" />
              Publish to HuggingFace...
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </div>
    </div>

    {#if exportSuccess}
      <div class="mx-5 mt-2 px-3 py-1.5 rounded-md text-[12px] {exportSuccess.startsWith('Export failed') ? 'bg-danger/10 border border-danger/30 text-danger' : 'bg-success/10 border border-success/30 text-success-hover'}">
        {exportSuccess}
      </div>
    {/if}

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
        <Tabs.Trigger
          value="backups"
          class="flex items-center gap-1.5 px-3 py-1.5 border-none bg-transparent text-text-tertiary text-[13px] font-medium cursor-pointer rounded-sm transition-all duration-[--transition-default] hover:bg-bg-surface hover:text-text-secondary data-[state=active]:bg-accent-hover data-[state=active]:text-text-primary"
          onclick={loadBackups}
        >
          <Archive size={14} />
          Backups
        </Tabs.Trigger>
      </Tabs.List>

      <div class="flex-1 overflow-hidden relative">
        <Tabs.Content value="conversation" class="h-full">
          <ConversationView {conversation} onbranch={handleBranchFromEvent} {branchingEventIndex} />
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

        <!-- Backups Tab -->
        <Tabs.Content value="backups" class="h-full overflow-y-auto p-6">
          <!-- Status messages -->
          {#if backupSuccess}
            <div class="mb-4 p-3 bg-success/10 border border-success/30 rounded-md text-[13px] text-success-hover flex items-center justify-between">
              <span>{backupSuccess}</span>
              <button
                class="text-text-muted hover:text-text-primary bg-transparent border-none cursor-pointer text-xs"
                onclick={() => backupSuccess = null}
              >&times;</button>
            </div>
          {/if}
          {#if backupError}
            <div class="mb-4 p-3 bg-danger/10 border border-danger/30 rounded-md text-[13px] text-danger flex items-center justify-between">
              <span>{backupError}</span>
              <button
                class="text-text-muted hover:text-text-primary bg-transparent border-none cursor-pointer text-xs"
                onclick={() => backupError = null}
              >&times;</button>
            </div>
          {/if}

          <!-- Actions -->
          <div class="flex flex-wrap gap-3 mb-6">
            <!-- Create Backup -->
            <div class="flex items-center gap-2 flex-1 min-w-[300px]">
              <input
                type="text"
                bind:value={backupLabel}
                placeholder="Backup label (optional)"
                class="flex-1 px-3 py-1.5 bg-bg-surface border border-border-default rounded-md text-[13px] text-text-primary placeholder:text-text-faint outline-none focus:border-accent-hover transition-colors"
                onkeydown={(e) => { if (e.key === "Enter") handleCreateBackup(); }}
              />
              <button
                class="flex items-center gap-1.5 px-3 py-1.5 bg-success border-none rounded-md text-text-primary text-[13px] cursor-pointer transition-colors hover:bg-success-hover disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
                onclick={handleCreateBackup}
                disabled={creatingBackup}
              >
                <Plus size={14} />
                {creatingBackup ? "Creating..." : "Create Backup"}
              </button>
            </div>

            <!-- Branch Full Conversation -->
            <button
              class="flex items-center gap-1.5 px-3 py-1.5 bg-accent-hover border-none rounded-md text-text-primary text-[13px] cursor-pointer transition-colors hover:bg-accent-active disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
              onclick={handleBranchFull}
              disabled={branchingFull}
              title="Duplicate this conversation with new IDs so both can continue independently"
            >
              <Copy size={14} />
              {branchingFull ? "Branching..." : "Duplicate Conversation"}
            </button>
          </div>

          <!-- Backup List -->
          <h3 class="text-text-secondary text-sm m-0 mb-3 font-semibold">
            Backups ({backups.length})
          </h3>

          {#if backups.length === 0}
            <div class="text-center py-12 text-text-muted">
              <Archive size={32} class="mx-auto mb-3 opacity-40" />
              <p class="m-0 text-sm">No backups yet</p>
              <p class="m-0 mt-1 text-xs text-text-faint">Create a backup to save the current conversation state</p>
            </div>
          {:else}
            <div class="flex flex-col gap-3">
              {#each backups.toReversed() as backup (backup.id)}
                <div class="bg-bg-surface border border-border-default rounded-lg p-4 transition-colors hover:border-border-strong">
                  <div class="flex items-start justify-between gap-3">
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center gap-2 mb-1">
                        {#if backup.auto_backup}
                          <Shield size={12} class="text-warning shrink-0" />
                        {:else}
                          <Archive size={12} class="text-accent-hover shrink-0" />
                        {/if}
                        <span class="text-[13px] font-medium text-text-primary truncate">
                          {backup.label}
                        </span>
                        {#if backup.auto_backup}
                          <span class="text-[10px] bg-warning/20 text-warning px-1.5 py-0.5 rounded-full font-medium uppercase tracking-wider">
                            auto
                          </span>
                        {/if}
                        {#if backup.truncated_at_event !== null}
                          <span class="text-[10px] bg-accent-hover/20 text-accent-hover px-1.5 py-0.5 rounded-full font-medium flex items-center gap-0.5">
                            <Scissors size={9} />
                            @{backup.truncated_at_event}
                          </span>
                        {/if}
                      </div>
                      <div class="flex items-center gap-3 text-xs text-text-faint">
                        <span class="inline-flex items-center gap-1">
                          <Clock size={10} />
                          {formatBackupDate(backup.created_at)}
                        </span>
                        <span>{backup.event_count} events</span>
                        <span>{formatBytes(backup.size_bytes)}</span>
                      </div>
                    </div>

                    <div class="flex items-center gap-1.5 shrink-0">
                      <button
                        class="flex items-center gap-1 px-2 py-1 bg-transparent border border-border-default rounded text-[11px] text-text-secondary cursor-pointer transition-colors hover:bg-bg-overlay hover:border-accent-hover hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed"
                        onclick={() => handleRestore(backup)}
                        disabled={restoringId === backup.id}
                        title="Restore conversation to this backup (auto-saves current state first)"
                      >
                        <RotateCcw size={11} />
                        {restoringId === backup.id ? "..." : "Restore"}
                      </button>
                      <button
                        class="flex items-center gap-1 px-2 py-1 bg-transparent border border-border-default rounded text-[11px] text-text-secondary cursor-pointer transition-colors hover:bg-bg-overlay hover:border-accent-hover hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed"
                        onclick={() => handleBranchFromBackup(backup)}
                        disabled={branchingBackupId === backup.id}
                        title="Create a new conversation from this backup with fresh IDs"
                      >
                        <GitBranch size={11} />
                        {branchingBackupId === backup.id ? "..." : "Branch"}
                      </button>
                      <button
                        class="flex items-center gap-1 px-2 py-1 bg-transparent border border-border-default rounded text-[11px] text-text-secondary cursor-pointer transition-colors hover:bg-danger/10 hover:border-danger/50 hover:text-danger disabled:opacity-50 disabled:cursor-not-allowed"
                        onclick={() => handleDelete(backup)}
                        disabled={deletingId === backup.id}
                        title="Delete this backup"
                      >
                        <Trash2 size={11} />
                        {deletingId === backup.id ? "..." : ""}
                      </button>
                    </div>
                  </div>
                </div>
              {/each}
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

{#if conversation}
  <PublishDialog
    bind:open={publishOpen}
    projectPaths={conversation.metadata.project ? [getProjectPath(conversation.metadata.file_path)] : []}
    defaultRepoName={cleanProjectName(conversation.metadata.project || "export") + "-training"}
  />
{/if}
