<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    projects,
    selectedProject,
    conversations,
    sidebarCollapsed,
    viewMode,
    selectedConversation,
    activeProvider,
    availableProviders,
  } from "$lib/stores";
  import {
    setLoading,
    setError,
    setMessage,
  } from "$lib/stores/statusStore";
  import { listProjects, listConversations, startIndexing, exportAllConversations, listProviders, setProvider } from "$lib/api";
  import { save } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import type { IndexingProgress, ExportFormat, ExportAllResult, Provider } from "$lib/types";
  import { FolderOpen, ChevronLeft, ChevronRight, FileDown, Loader2, RefreshCw } from "lucide-svelte";
  import { DropdownMenu } from "bits-ui";
  import Skeleton from "./Skeleton.svelte";

  let loading = $state(false);
  let error: string | null = $state(null);
  let unlisten: (() => void) | null = null;
  let filterQuery = $state("");
  let exportingAll = $state(false);
  let exportMsg = $state<string | null>(null);

  async function handleExportAllProjects(format: ExportFormat) {
    if (exportingAll) return;
    exportingAll = true;
    exportMsg = null;

    try {
      let ext = "jsonl";
      if (format === "ShareGPT") ext = "json";

      const outputPath = await save({
        defaultPath: `all_projects_${format.toLowerCase()}.${ext}`,
        filters: [{ name: ext === "json" ? "JSON Files" : "JSONL Files", extensions: [ext] }],
      });

      if (!outputPath) {
        exportingAll = false;
        return;
      }

      // Empty array = all projects
      const result: ExportAllResult = await exportAllConversations([], format, outputPath);
      exportMsg = `Exported ${result.conversations_exported} conversations${result.conversations_skipped > 0 ? ` (${result.conversations_skipped} skipped)` : ""}`;
      setTimeout(() => { exportMsg = null; }, 5000);
    } catch (e) {
      exportMsg = `Export failed: ${e}`;
      setTimeout(() => { exportMsg = null; }, 5000);
    } finally {
      exportingAll = false;
    }
  }

  let filteredProjects = $derived(
    filterQuery
      ? $projects.filter((p) =>
          formatProjectName(p.name).toLowerCase().includes(filterQuery.toLowerCase()) ||
          p.name.toLowerCase().includes(filterQuery.toLowerCase())
        )
      : $projects
  );

  async function switchProvider(provider: Provider) {
    if (provider === $activeProvider) return;
    try {
      // Clear all state before switching to avoid stale data races
      selectedConversation.set(null);
      conversations.set([]);
      selectedProject.set(null);
      projects.set([]);

      await setProvider(provider);
      activeProvider.set(provider);
      await loadProjects();
    } catch (e) {
      console.error("Failed to switch provider:", e);
    }
  }

  onMount(async () => {
    // Detect available providers
    try {
      const providers = await listProviders();
      const available = providers.filter(p => p.available).map(p => p.provider);
      if (available.length > 0) {
        availableProviders.set(available);
        activeProvider.set(available[0]);
      }
    } catch (e) {
      console.error("Failed to list providers:", e);
    }

    await loadProjects();

    const unlistenProgress = await listen<IndexingProgress>(
      "indexing-progress",
      (event) => {
        const progress = event.payload;
        setLoading(progress.status, {
          current: progress.current,
          total: progress.total,
        });
      }
    );

    const unlistenComplete = await listen("indexing-complete", () => {
      setMessage("Search index ready!");
    });

    const unlistenError = await listen<string>(
      "indexing-error",
      (event) => {
        setError(`Indexing error: ${event.payload}`);
      }
    );

    const unlistenChanged = await listen<string[]>(
      "conversation-changed",
      async () => {
        if ($selectedProject) {
          try {
            const convos = await listConversations($selectedProject.path);
            conversations.set(convos);
          } catch (e) {
            console.error("Failed to refresh conversations:", e);
          }
        }
      }
    );

    unlisten = () => {
      unlistenProgress();
      unlistenComplete();
      unlistenError();
      unlistenChanged();
    };
  });

  onDestroy(() => {
    if (unlisten) unlisten();
  });

  async function loadProjects() {
    loading = true;
    error = null;
    setLoading("Loading projects...");
    try {
      const projectList = await listProjects();
      projects.set(projectList);
      if (projectList.length > 0) {
        await selectProject(projectList[0]);
        const projectPaths = projectList.map((p) => p.path);
        await startIndexing(projectPaths);
      } else {
        setMessage("No projects found");
      }
    } catch (e) {
      error = `Failed to load projects: ${e}`;
      setError(`Failed to load projects: ${e}`);
      console.error(e);
    } finally {
      loading = false;
    }
  }

  function formatProjectName(name: string): string {
    // Transform path-style names like "-Users-nick-work-myproject" to "myproject"
    // or "my-cool-project" stays as "my-cool-project"
    const cleaned = name.replace(/^-/, "");
    const segments = cleaned.split("-");

    // If it looks like a path (starts with Users/home), take the last meaningful segment
    if (
      segments.length > 2 &&
      (segments[0].toLowerCase() === "users" || segments[0].toLowerCase() === "home")
    ) {
      // Return last segment, which is typically the project folder name
      return segments[segments.length - 1] || name;
    }

    return name;
  }

  async function selectProject(project: (typeof $projects)[0]) {
    selectedProject.set(project);
    selectedConversation.set(null);
    viewMode.set("list");
    setLoading(`Loading conversations from ${formatProjectName(project.name)}...`);
    try {
      const convos = await listConversations(project.path);
      conversations.set(convos);
      setMessage(
        `Loaded ${convos.length} conversations from ${formatProjectName(project.name)}`
      );
    } catch (e) {
      error = `Failed to load conversations: ${e}`;
      setError(`Failed to load conversations: ${e}`);
      console.error(e);
    }
  }
</script>

<aside
  class="flex flex-col h-full bg-bg-base border-r border-border-default transition-[width] duration-[--transition-default]"
  class:w-[280px]={!$sidebarCollapsed}
  class:w-[50px]={$sidebarCollapsed}
>
  <div class="flex items-center justify-between p-4 border-b border-border-default">
    {#if !$sidebarCollapsed}
      <div class="flex items-center gap-2">
        {#if $availableProviders.length > 1}
          <div class="flex gap-0.5 bg-bg-surface rounded-md p-0.5">
            {#each $availableProviders as p}
              <button
                class="px-2.5 py-0.5 text-xs rounded cursor-pointer border-none transition-colors duration-[--transition-fast]"
                class:bg-accent={$activeProvider === p}
                class:text-text-primary={$activeProvider === p}
                class:bg-transparent={$activeProvider !== p}
                class:text-text-muted={$activeProvider !== p}
                onclick={() => switchProvider(p)}
              >{p}</button>
            {/each}
          </div>
        {:else}
          <h2 class="m-0 text-lg font-semibold text-text-primary">Projects</h2>
        {/if}
      </div>
    {/if}
    <div class="flex items-center gap-1">
      {#if !$sidebarCollapsed}
        <button
          class="p-1 text-text-muted hover:text-text-primary bg-transparent border-none cursor-pointer transition-colors duration-[--transition-fast]"
          onclick={loadProjects}
          disabled={loading}
          aria-label="Refresh projects"
          title="Rescan for new projects"
        >
          <RefreshCw size={15} class={loading ? "animate-spin" : ""} />
        </button>
      {/if}
      <button
        class="p-1 text-text-muted hover:text-text-primary bg-transparent border-none cursor-pointer transition-colors duration-[--transition-fast]"
        onclick={() => sidebarCollapsed.update((v) => !v)}
        aria-label="Toggle sidebar"
      >
        {#if $sidebarCollapsed}
          <ChevronRight size={18} />
        {:else}
          <ChevronLeft size={18} />
        {/if}
      </button>
    </div>
  </div>

  {#if !$sidebarCollapsed}
    <div class="px-3 py-2 flex flex-col gap-2 border-b border-border-default">
      <!-- Export All Projects -->
      <DropdownMenu.Root>
        <DropdownMenu.Trigger
          class="flex items-center justify-center gap-2 w-full px-3 py-2 bg-bg-surface border border-border-default rounded-md text-sm text-text-secondary cursor-pointer transition-colors hover:bg-bg-overlay hover:text-text-primary hover:border-accent-hover disabled:opacity-50 disabled:cursor-not-allowed"
          disabled={exportingAll}
        >
          {#if exportingAll}
            <Loader2 size={14} class="animate-spin text-accent-hover" />
            Exporting...
          {:else}
            <FileDown size={14} />
            Export All Projects
          {/if}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content
          class="min-w-[180px] mt-1 bg-bg-elevated border border-border-default rounded-md shadow-lg z-50 py-1"
        >
          {@const itemClass = "flex items-center gap-2 w-full px-3 py-2 text-left text-[13px] text-text-secondary cursor-pointer bg-transparent border-none hover:bg-bg-overlay hover:text-text-primary transition-colors"}
          <DropdownMenu.Item class={itemClass} onSelect={() => handleExportAllProjects("ChatML")}>
            ChatML (text only)
          </DropdownMenu.Item>
          <DropdownMenu.Item class={itemClass} onSelect={() => handleExportAllProjects("ChatMLTools")}>
            ChatML + Tools (agentic)
          </DropdownMenu.Item>
          <DropdownMenu.Item class={itemClass} onSelect={() => handleExportAllProjects("ShareGPT")}>
            ShareGPT
          </DropdownMenu.Item>
          <DropdownMenu.Item class={itemClass} onSelect={() => handleExportAllProjects("Alpaca")}>
            Alpaca
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>

      {#if exportMsg}
        <div class="text-[11px] text-success-hover">{exportMsg}</div>
      {/if}

      <!-- Filter -->
      <input
        type="text"
        bind:value={filterQuery}
        placeholder="Filter projects..."
        class="w-full px-3 py-1.5 text-sm bg-bg-surface border border-border-default rounded-md text-text-secondary placeholder:text-text-faint focus:outline-none focus:border-accent"
      />
    </div>
  {/if}

  {#if loading}
    <div class="flex-1 p-3 space-y-3">
      {#if !$sidebarCollapsed}
        {#each { length: 5 } as _}
          <div class="p-3 rounded-lg bg-bg-surface">
            <Skeleton height="14px" width="70%" />
            <div class="mt-2">
              <Skeleton height="12px" width="40%" />
            </div>
          </div>
        {/each}
      {/if}
    </div>
  {:else if error}
    <div class="p-4 text-center text-error text-sm">{error}</div>
  {:else if filteredProjects.length === 0}
    <div class="p-4 text-center text-text-muted text-sm">
      {filterQuery ? "No matching projects" : "No projects found"}
    </div>
  {:else}
    <ul class="list-none p-0 m-0 overflow-y-auto flex-1">
      {#each filteredProjects as project (project.name)}
        <li>
          <button
            class="flex items-start gap-2.5 w-full text-left p-3 cursor-pointer border-none border-b border-border-muted bg-transparent text-inherit font-inherit transition-colors duration-[--transition-default] hover:bg-bg-surface"
            class:bg-bg-elevated={$selectedProject?.name === project.name}
            class:border-l-[3px]={$selectedProject?.name === project.name}
            class:border-l-accent={$selectedProject?.name === project.name}
            onclick={() => selectProject(project)}
          >
            <FolderOpen
              size={16}
              class="mt-0.5 shrink-0 text-text-muted"
            />
            {#if !$sidebarCollapsed}
              <div class="min-w-0">
                <div class="font-medium text-text-primary text-sm break-all leading-snug">
                  {formatProjectName(project.name)}
                </div>
                <div class="text-xs text-text-muted mt-0.5">
                  {project.conversation_count} conversations
                </div>
              </div>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</aside>
