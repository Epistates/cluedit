<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    projects,
    selectedProject,
    conversations,
    sidebarCollapsed,
    viewMode,
    selectedConversation,
  } from "$lib/stores";
  import {
    setLoading,
    setError,
    setMessage,
  } from "$lib/stores/statusStore";
  import { listProjects, listConversations, startIndexing } from "$lib/api";
  import { listen } from "@tauri-apps/api/event";
  import type { IndexingProgress } from "$lib/types";
  import { FolderOpen, ChevronLeft, ChevronRight } from "lucide-svelte";
  import Skeleton from "./Skeleton.svelte";

  let loading = $state(false);
  let error: string | null = $state(null);
  let unlisten: (() => void) | null = null;
  let filterQuery = $state("");

  let filteredProjects = $derived(
    filterQuery
      ? $projects.filter((p) =>
          formatProjectName(p.name).toLowerCase().includes(filterQuery.toLowerCase()) ||
          p.name.toLowerCase().includes(filterQuery.toLowerCase())
        )
      : $projects
  );

  onMount(async () => {
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
  <div class="flex items-center justify-between p-4 pt-2 border-b border-border-default">
    {#if !$sidebarCollapsed}
      <h2 class="m-0 text-lg font-semibold text-text-primary">Projects</h2>
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

  {#if !$sidebarCollapsed}
    <div class="px-3 py-2">
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
