<script lang="ts">
  import {
    viewMode,
    sortBy,
    sortOrder,
    showEmptyConversations,
    dateFilterFrom,
    dateFilterTo,
    selectedProject,
  } from "$lib/stores";
  import { exportAllConversations } from "$lib/api";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import type { ExportFormat, ExportAllResult } from "$lib/types";
  import { DropdownMenu } from "bits-ui";
  import {
    LayoutList,
    Search,
    ArrowUpDown,
    ArrowUp,
    ArrowDown,
    Calendar,
    Eye,
    EyeOff,
    FileDown,
    ChevronDown,
    FileJson,
    Braces,
    FileText,
    FileType,
    GraduationCap,
    Database,
    BookOpen,
    Upload,
  } from "lucide-svelte";
  import PublishDialog from "./PublishDialog.svelte";

  let exportingAll = $state(false);
  let exportResult = $state<string | null>(null);
  let publishOpen = $state(false);

  function toggleView(mode: "list" | "search") {
    viewMode.set(mode);
  }

  const TRAINING_FORMATS: ExportFormat[] = ["ChatML", "ChatMLTools", "Alpaca", "ShareGPT"];

  /** Clean up Claude project directory names for display/filenames */
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

  async function handleExport(format: ExportFormat, allProjects: boolean) {
    if (exportingAll) return;

    const project = $selectedProject;
    // For single-project export, need a selected project
    if (!allProjects && !project) return;

    exportingAll = true;
    exportResult = null;

    try {
      const isTraining = TRAINING_FORMATS.includes(format);
      const label = allProjects ? "all_projects" : cleanProjectName(project!.name);

      let outputPath: string | null = null;

      if (isTraining) {
        let ext = "jsonl";
        let filterName = "JSONL Files";
        if (format === "ShareGPT") {
          ext = "json";
          filterName = "JSON Files";
        }
        outputPath = await save({
          defaultPath: `${label}_${format.toLowerCase()}.${ext}`,
          filters: [{ name: filterName, extensions: [ext] }],
        });
      } else {
        outputPath = await open({
          directory: true,
          title: `Select output directory for ${format} export`,
        }) as string | null;
      }

      if (!outputPath) {
        exportingAll = false;
        return;
      }

      // Empty array = all projects
      const projectPaths = allProjects ? [] : [project!.path];

      const result: ExportAllResult = await exportAllConversations(
        projectPaths,
        format,
        outputPath
      );

      const scope = allProjects ? "all projects" : cleanProjectName(project!.name);
      exportResult = `Exported ${result.conversations_exported} conversations from ${scope}${result.conversations_skipped > 0 ? ` (${result.conversations_skipped} skipped)` : ""}`;
      setTimeout(() => { exportResult = null; }, 5000);
    } catch (e) {
      exportResult = `Export failed: ${e}`;
      setTimeout(() => { exportResult = null; }, 5000);
    } finally {
      exportingAll = false;
    }
  }
</script>

<div
  class="h-12 bg-bg-base border-b border-border-default flex items-center justify-between px-4 gap-3"
>
  <div class="flex items-center gap-1.5">
    <button
      class="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm border border-transparent cursor-pointer transition-all duration-[--transition-fast]"
      class:bg-accent={$viewMode === "list" || $viewMode === "detail"}
      class:text-text-primary={$viewMode === "list" || $viewMode === "detail"}
      class:bg-transparent={$viewMode === "search"}
      class:text-text-secondary={$viewMode === "search"}
      class:border-border-default={$viewMode === "search"}
      onclick={() => toggleView("list")}
    >
      <LayoutList size={15} />
      Conversations
    </button>
    <button
      class="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm border border-transparent cursor-pointer transition-all duration-[--transition-fast]"
      class:bg-accent={$viewMode === "search"}
      class:text-text-primary={$viewMode === "search"}
      class:bg-transparent={$viewMode !== "search"}
      class:text-text-secondary={$viewMode !== "search"}
      class:border-border-default={$viewMode !== "search"}
      onclick={() => toggleView("search")}
    >
      <Search size={15} />
      Search
    </button>
  </div>

  {#if $viewMode === "list"}
    <div class="flex items-center gap-2">
      <!-- Export Project dropdown -->
      {#if $selectedProject}
        <DropdownMenu.Root>
          <DropdownMenu.Trigger
            class="flex items-center gap-1 px-2 py-1 bg-transparent border border-border-default rounded-md text-xs text-text-secondary cursor-pointer transition-colors hover:bg-bg-surface hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed"
            disabled={exportingAll}
          >
            <FileDown size={13} />
            {exportingAll ? "Exporting..." : "Export Project"}
            <ChevronDown size={10} />
          </DropdownMenu.Trigger>
          <DropdownMenu.Content
            class="min-w-[200px] mt-1 bg-bg-elevated border border-border-default rounded-md shadow-lg z-10 py-1"
          >
            <div class="px-3 py-1 text-[11px] text-text-faint uppercase tracking-wider font-medium">
              This Project — Training
            </div>
            {@const itemClass = "flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"}
            <DropdownMenu.Item class={itemClass} onSelect={() => handleExport("ChatML", false)}>
              <GraduationCap size={14} class="text-text-muted" /> ChatML (text only)
            </DropdownMenu.Item>
            <DropdownMenu.Item class={itemClass} onSelect={() => handleExport("ChatMLTools", false)}>
              <GraduationCap size={14} class="text-accent-hover" /> ChatML + Tools
            </DropdownMenu.Item>
            <DropdownMenu.Item class={itemClass} onSelect={() => handleExport("ShareGPT", false)}>
              <Database size={14} class="text-text-muted" /> ShareGPT
            </DropdownMenu.Item>
            <DropdownMenu.Item class={itemClass} onSelect={() => handleExport("Alpaca", false)}>
              <BookOpen size={14} class="text-text-muted" /> Alpaca
            </DropdownMenu.Item>
            <DropdownMenu.Separator class="h-px bg-border-default my-1" />
            <div class="px-3 py-1 text-[11px] text-text-faint uppercase tracking-wider font-medium">
              This Project — Standard
            </div>
            <DropdownMenu.Item class={itemClass} onSelect={() => handleExport("JsonPretty", false)}>
              <FileJson size={14} class="text-text-muted" /> JSON
            </DropdownMenu.Item>
            <DropdownMenu.Item class={itemClass} onSelect={() => handleExport("Markdown", false)}>
              <FileText size={14} class="text-text-muted" /> Markdown
            </DropdownMenu.Item>
            <DropdownMenu.Item class={itemClass} onSelect={() => handleExport("Text", false)}>
              <FileType size={14} class="text-text-muted" /> Plain Text
            </DropdownMenu.Item>
            <DropdownMenu.Separator class="h-px bg-border-default my-1" />
            <DropdownMenu.Item class={itemClass} onSelect={() => { publishOpen = true; }}>
              <Upload size={14} class="text-accent-hover" /> Publish to HuggingFace...
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      {/if}

      <div class="flex items-center gap-1">
        <Calendar size={14} class="text-text-muted" />
        <input
          type="date"
          bind:value={$dateFilterFrom}
          title="From date"
          class="bg-bg-elevated border border-border-default text-text-secondary px-2 py-1 rounded-md text-xs cursor-pointer focus:outline-none focus:border-accent [&::-webkit-calendar-picker-indicator]:invert-[0.7]"
        />
        <input
          type="date"
          bind:value={$dateFilterTo}
          title="To date"
          class="bg-bg-elevated border border-border-default text-text-secondary px-2 py-1 rounded-md text-xs cursor-pointer focus:outline-none focus:border-accent [&::-webkit-calendar-picker-indicator]:invert-[0.7]"
        />
      </div>

      <button
        class="flex items-center gap-1.5 px-2 py-1 rounded-md text-xs text-text-secondary cursor-pointer border-none bg-transparent hover:bg-bg-surface transition-colors duration-[--transition-fast]"
        onclick={() => showEmptyConversations.update((v) => !v)}
        title={$showEmptyConversations ? "Hide empty conversations" : "Show empty conversations"}
      >
        {#if $showEmptyConversations}
          <Eye size={14} />
        {:else}
          <EyeOff size={14} />
        {/if}
        <span class="hidden sm:inline">Empty</span>
      </button>

      <select
        bind:value={$sortBy}
        class="bg-bg-elevated border border-border-default text-text-secondary px-2 py-1 rounded-md text-xs cursor-pointer focus:outline-none focus:border-accent"
      >
        <option value="modified">Modified</option>
        <option value="created">Created</option>
        <option value="size">Size</option>
      </select>

      <button
        class="flex items-center justify-center w-8 h-8 bg-bg-elevated border border-border-default text-text-secondary rounded-md cursor-pointer hover:border-accent hover:text-text-primary transition-all duration-[--transition-fast]"
        onclick={() =>
          sortOrder.set($sortOrder === "asc" ? "desc" : "asc")}
        title={$sortOrder === "asc" ? "Sort descending" : "Sort ascending"}
      >
        {#if $sortOrder === "asc"}
          <ArrowUp size={14} />
        {:else}
          <ArrowDown size={14} />
        {/if}
      </button>
    </div>
  {/if}
</div>

{#if exportResult}
  <div class="h-7 bg-success/10 border-b border-success/30 flex items-center px-4 text-xs text-success-hover">
    {exportResult}
  </div>
{/if}

<PublishDialog
  bind:open={publishOpen}
  projectPaths={$selectedProject ? [$selectedProject.path] : []}
  defaultRepoName={$selectedProject ? cleanProjectName($selectedProject.name) + "-training" : "training-data"}
/>
