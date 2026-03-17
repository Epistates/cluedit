<script lang="ts">
  import {
    viewMode,
    sortBy,
    sortOrder,
    showEmptyConversations,
    dateFilterFrom,
    dateFilterTo,
  } from "$lib/stores";
  import {
    LayoutList,
    Search,
    ArrowUpDown,
    ArrowUp,
    ArrowDown,
    Calendar,
    Eye,
    EyeOff,
  } from "lucide-svelte";

  function toggleView(mode: "list" | "search") {
    viewMode.set(mode);
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
