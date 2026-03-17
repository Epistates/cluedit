<script lang="ts">
  import { Loader2 } from "lucide-svelte";

  let {
    loading = false,
    message = "",
    progress = null,
    error = null,
  }: {
    loading?: boolean;
    message?: string;
    progress?: { current: number; total: number } | null;
    error?: string | null;
  } = $props();

  let displayMessage = $state("");
  let displayError: string | null = $state(null);
  let timeout: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    displayMessage = message;
    displayError = error;

    if (message && !loading) {
      if (timeout) clearTimeout(timeout);
      timeout = setTimeout(() => {
        displayMessage = "";
        displayError = null;
      }, 5000);
    }
  });
</script>

<div
  class="flex justify-between items-center h-7 px-4 bg-bg-base border-t text-xs"
  class:border-border-default={!displayError}
  class:bg-error-bg={displayError}
  class:border-error-border={displayError}
>
  <div class="flex items-center gap-2">
    {#if loading}
      <Loader2 size={14} class="animate-spin text-accent-hover" />
      <span class="text-text-secondary">{displayMessage || "Loading..."}</span>
      {#if progress}
        <span class="text-text-muted tabular-nums">
          ({progress.current}/{progress.total})
        </span>
      {/if}
    {:else if displayError}
      <span class="text-error">{displayError}</span>
    {:else if displayMessage}
      <span class="text-text-secondary">{displayMessage}</span>
    {:else}
      <span class="text-text-faint">Ready</span>
    {/if}
  </div>

  <div class="flex items-center gap-3">
    {#if progress}
      <div class="w-[100px] h-1 bg-bg-muted rounded-full overflow-hidden">
        <div
          class="h-full bg-accent-hover transition-[width] duration-[--transition-default] rounded-full"
          style="width: {(progress.current / progress.total) * 100}%"
        ></div>
      </div>
    {/if}
  </div>
</div>
