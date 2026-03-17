<script lang="ts">
  import { Minus, Square, X } from "lucide-svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  let isMac = $state(false);

  onMount(() => {
    isMac = navigator.userAgent.includes("Mac");
  });

  async function minimize() {
    await getCurrentWindow().minimize();
  }

  async function toggleMaximize() {
    await getCurrentWindow().toggleMaximize();
  }

  async function close() {
    await getCurrentWindow().close();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  data-tauri-drag-region
  class="fixed top-0 left-0 right-0 z-50 flex items-center h-[--spacing-titlebar-h] bg-bg-base border-b border-border-default select-none"
  style:padding-left={isMac ? "78px" : "12px"}
>
  <span
    data-tauri-drag-region
    class="text-sm font-semibold text-text-tertiary tracking-wide pointer-events-none"
  >
    ClueEdit
  </span>

  {#if !isMac}
    <div class="ml-auto flex items-center">
      <button
        onclick={minimize}
        class="flex items-center justify-center w-[46px] h-[--spacing-titlebar-h] text-text-muted hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
        aria-label="Minimize"
      >
        <Minus size={16} />
      </button>
      <button
        onclick={toggleMaximize}
        class="flex items-center justify-center w-[46px] h-[--spacing-titlebar-h] text-text-muted hover:bg-bg-overlay hover:text-text-primary transition-colors duration-[--transition-fast]"
        aria-label="Maximize"
      >
        <Square size={14} />
      </button>
      <button
        onclick={close}
        class="flex items-center justify-center w-[46px] h-[--spacing-titlebar-h] text-text-muted hover:bg-red-600 hover:text-text-primary transition-colors duration-[--transition-fast]"
        aria-label="Close"
      >
        <X size={16} />
      </button>
    </div>
  {/if}
</div>
