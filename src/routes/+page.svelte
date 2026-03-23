<script lang="ts">
  import {
    viewMode,
    selectedConversation,
    commandPaletteOpen,
  } from "$lib/stores";
  import { statusStore } from "$lib/stores/statusStore";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Toolbar from "$lib/components/Toolbar.svelte";
  import ConversationList from "$lib/components/ConversationList.svelte";
  import ConversationViewer from "$lib/components/ConversationViewer.svelte";
  import SearchView from "$lib/components/SearchView.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import { fade } from "svelte/transition";

  function handleKeydown(event: KeyboardEvent) {
    const isMod = event.metaKey || event.ctrlKey;

    if (isMod && event.key === "k") {
      event.preventDefault();
      commandPaletteOpen.update((v) => !v);
    } else if (event.key === "Escape") {
      if ($commandPaletteOpen) {
        commandPaletteOpen.set(false);
      } else if ($viewMode === "detail") {
        viewMode.set("list");
      } else if ($viewMode === "search") {
        viewMode.set("list");
      }
    } else if (isMod && event.key === "e") {
      event.preventDefault();
      if ($selectedConversation) {
        viewMode.set("detail");
      }
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="flex h-screen overflow-hidden">
  <Sidebar />
  <div class="flex-1 flex flex-col overflow-hidden">
    <Toolbar />
    <div class="flex-1 flex overflow-hidden">
      {#key $viewMode}
        <div class="flex-1 flex overflow-hidden" transition:fade={{ duration: 100 }}>
          {#if $viewMode === "list"}
            <ConversationList />
          {:else if $viewMode === "detail"}
            <ConversationViewer />
          {:else if $viewMode === "search"}
            <SearchView />
          {/if}
        </div>
      {/key}
    </div>
    <StatusBar
      loading={$statusStore.loading}
      message={$statusStore.message}
      progress={$statusStore.progress}
      error={$statusStore.error}
    />
  </div>
</div>

<CommandPalette />
