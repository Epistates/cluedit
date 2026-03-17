<script lang="ts">
  import {
    ChevronsDown,
    ChevronsUp,
    Copy,
    Check,
    ChevronRight,
    ChevronDown,
    Search,
    Clipboard,
  } from "lucide-svelte";

  let { data }: { data: unknown } = $props();

  let expandedPaths = $state(new Set<string>());
  let searchQuery = $state("");
  let copiedPath: string | null = $state(null);
  let copiedJson = $state(false);

  function togglePath(path: string) {
    const next = new Set(expandedPaths);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.add(path);
    }
    expandedPaths = next;
  }

  function isExpanded(path: string): boolean {
    return expandedPaths.has(path);
  }

  function copyPath(path: string, event: Event) {
    event.stopPropagation();
    navigator.clipboard.writeText(path);
    copiedPath = path;
    setTimeout(() => (copiedPath = null), 2000);
  }

  function copyValue(value: unknown, event: Event) {
    event.stopPropagation();
    navigator.clipboard.writeText(JSON.stringify(value, null, 2));
    copiedJson = true;
    setTimeout(() => (copiedJson = false), 2000);
  }

  function expandAll() {
    const paths: string[] = [];
    function traverse(obj: unknown, path = "") {
      if (obj && typeof obj === "object") {
        paths.push(path);
        for (const key in obj as Record<string, unknown>) {
          traverse(
            (obj as Record<string, unknown>)[key],
            path ? `${path}.${key}` : key
          );
        }
      }
    }
    traverse(data);
    expandedPaths = new Set(paths);
  }

  function collapseAll() {
    expandedPaths = new Set();
  }

  function getValueType(value: unknown): string {
    if (value === null) return "null";
    if (Array.isArray(value)) return "array";
    return typeof value;
  }

  function formatValue(value: unknown): string {
    if (value === null) return "null";
    if (typeof value === "string") return `"${value}"`;
    if (typeof value === "boolean") return value.toString();
    if (typeof value === "number") return value.toString();
    return "";
  }

  interface JsonNode {
    key: string;
    value: unknown;
    path: string;
    depth: number;
    isExpandable: boolean;
    expanded: boolean;
    type: string;
    matches: boolean;
  }

  function matchesSearch(path: string, value: unknown): boolean {
    if (!searchQuery) return true;
    const query = searchQuery.toLowerCase();
    const pathMatches = path.toLowerCase().includes(query);
    const valueMatches =
      typeof value === "string" && value.toLowerCase().includes(query);
    return pathMatches || valueMatches;
  }

  function renderNode(
    obj: unknown,
    path = "",
    depth = 0
  ): JsonNode[] {
    if (!obj || typeof obj !== "object") {
      return [];
    }

    const entries = Object.entries(obj as Record<string, unknown>);
    const result: JsonNode[] = [];

    for (const [key, value] of entries) {
      const currentPath = path ? `${path}.${key}` : key;
      const isExpandable =
        value !== null &&
        typeof value === "object" &&
        Object.keys(value as object).length > 0;
      const expanded = isExpanded(currentPath);
      const type = getValueType(value);
      const matches = matchesSearch(currentPath, value);

      if (searchQuery && !matches) continue;

      result.push({
        key,
        value,
        path: currentPath,
        depth,
        isExpandable,
        expanded,
        type,
        matches,
      });

      if (isExpandable && expanded) {
        result.push(...renderNode(value, currentPath, depth + 1));
      }
    }

    return result;
  }

  let nodes = $derived(renderNode(data));
</script>

<div class="flex flex-col h-full bg-bg-base">
  <div class="flex gap-3 p-3 border-b border-border-default bg-bg-surface">
    <div class="flex-1 relative">
      <Search
        size={14}
        class="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted pointer-events-none"
      />
      <input
        type="text"
        placeholder="Search JSON..."
        bind:value={searchQuery}
        class="w-full pl-9 pr-3 py-2 bg-bg-overlay border border-border-strong rounded-md text-text-secondary text-[13px] font-inherit focus:outline-none focus:border-accent-hover"
      />
    </div>
    <div class="flex gap-2">
      <button
        class="flex items-center gap-1.5 px-3 py-2 bg-bg-overlay border border-border-strong rounded-md text-text-secondary text-xs cursor-pointer transition-all duration-[--transition-default] hover:bg-bg-elevated hover:border-accent-hover"
        onclick={expandAll}
        title="Expand All"
      >
        <ChevronsDown size={14} />
        Expand
      </button>
      <button
        class="flex items-center gap-1.5 px-3 py-2 bg-bg-overlay border border-border-strong rounded-md text-text-secondary text-xs cursor-pointer transition-all duration-[--transition-default] hover:bg-bg-elevated hover:border-accent-hover"
        onclick={collapseAll}
        title="Collapse All"
      >
        <ChevronsUp size={14} />
        Collapse
      </button>
      <button
        class="flex items-center gap-1.5 px-3 py-2 bg-bg-overlay border border-border-strong rounded-md text-text-secondary text-xs cursor-pointer transition-all duration-[--transition-default] hover:bg-bg-elevated hover:border-accent-hover"
        onclick={(e) => copyValue(data, e)}
        title="Copy JSON"
      >
        {#if copiedJson}
          <Check size={14} class="text-success-hover" />
          Copied
        {:else}
          <Copy size={14} />
          Copy
        {/if}
      </button>
    </div>
  </div>

  <div class="flex-1 overflow-y-auto py-2 font-mono text-[13px] leading-relaxed">
    {#each nodes as node (node.path)}
      <button
        class="flex items-center gap-1.5 py-1 px-2 cursor-pointer w-full text-left bg-transparent border-none text-inherit font-inherit transition-colors duration-[--transition-fast] hover:bg-bg-surface"
        class:bg-[#3d3d1f]={searchQuery && node.matches}
        style="padding-left: {node.depth * 16 + 8}px"
        onclick={() => node.isExpandable && togglePath(node.path)}
      >
        {#if node.isExpandable}
          {#if node.expanded}
            <ChevronDown size={12} class="text-text-tertiary shrink-0" />
          {:else}
            <ChevronRight size={12} class="text-text-tertiary shrink-0" />
          {/if}
        {:else}
          <span class="w-3 inline-block shrink-0"></span>
        {/if}

        <span class="text-syntax-property font-medium">{node.key}:</span>

        {#if node.isExpandable}
          <span class="text-text-muted italic">
            {node.type === "array" ? "[]" : "{}"}
          </span>
          <span class="text-text-muted text-xs ml-1">
            {Object.keys(node.value as object).length} items
          </span>
        {:else}
          <span
            class:text-syntax-string={node.type === "string"}
            class:text-syntax-number={node.type === "number"}
            class:text-syntax-keyword={node.type === "boolean" || node.type === "null"}
          >
            {formatValue(node.value)}
          </span>
        {/if}

        <span
          class="opacity-0 group-hover:opacity-100 ml-auto p-0.5 rounded-sm text-text-muted cursor-pointer transition-opacity hover:bg-bg-overlay hover:text-text-secondary"
          role="button"
          tabindex="0"
          onclick={(e) => copyPath(node.path, e)}
          onkeydown={(e) => e.key === "Enter" && copyPath(node.path, e)}
          title="Copy path"
        >
          {#if copiedPath === node.path}
            <Check size={12} class="text-success-hover" />
          {:else}
            <Clipboard size={12} />
          {/if}
        </span>
      </button>
    {/each}

    {#if nodes.length === 0 && searchQuery}
      <div class="p-8 text-center text-text-tertiary italic">
        No matches found for "{searchQuery}"
      </div>
    {/if}
  </div>
</div>
