<script lang="ts">
  let {
    class: className = "",
    width = "100%",
    height = "1rem",
  }: {
    class?: string;
    width?: string;
    height?: string;
  } = $props();
</script>

<div
  class="animate-pulse rounded-md bg-bg-muted {className}"
  style="width: {width}; height: {height};"
  aria-hidden="true"
></div>
