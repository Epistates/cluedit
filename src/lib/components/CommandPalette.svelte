<script lang="ts">
  import {
    commandPaletteOpen,
    viewMode,
    sidebarCollapsed,
    selectedConversation,
    filteredConversations,
  } from "$lib/stores";
  import { readConversation } from "$lib/api";
  import { setLoading, setMessage, setError } from "$lib/stores/statusStore";
  import { Dialog, Command } from "bits-ui";
  import {
    Search,
    LayoutList,
    PanelLeftClose,
    MessageSquare,
  } from "lucide-svelte";
  import type { ConversationMetadata } from "$lib/types";

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  type IconComponent = any;

  interface ActionItem {
    id: string;
    label: string;
    group: "actions" | "conversations";
    icon: IconComponent;
    action: () => void;
  }

  function buildActions(): ActionItem[] {
    return [
      {
        id: "action-search",
        label: "Open search",
        group: "actions",
        icon: Search,
        action: () => {
          viewMode.set("search");
          commandPaletteOpen.set(false);
        },
      },
      {
        id: "action-conversations",
        label: "View conversations",
        group: "actions",
        icon: LayoutList,
        action: () => {
          viewMode.set("list");
          commandPaletteOpen.set(false);
        },
      },
      {
        id: "action-toggle-sidebar",
        label: "Toggle sidebar",
        group: "actions",
        icon: PanelLeftClose,
        action: () => {
          sidebarCollapsed.update((v) => !v);
          commandPaletteOpen.set(false);
        },
      },
    ];
  }

  let actions = $derived(buildActions());

  let recentConversations = $derived(
    $filteredConversations.slice(0, 8).map(
      (conv): ActionItem => ({
        id: `conv-${conv.id}`,
        label:
          conv.title ||
          (conv.first_message
            ? conv.first_message.slice(0, 60)
            : `Conversation ${conv.id.slice(0, 8)}`),
        group: "conversations",
        icon: MessageSquare,
        action: () => openConversation(conv),
      })
    )
  );

  async function openConversation(metadata: ConversationMetadata) {
    commandPaletteOpen.set(false);
    try {
      setLoading("Opening conversation...");
      const conversation = await readConversation(metadata.file_path);
      selectedConversation.set(conversation);
      viewMode.set("detail");
      setMessage("Conversation loaded");
    } catch (e) {
      setError(`Failed to open conversation: ${e}`);
    }
  }

  function handleSelect(value: string) {
    const all = [...actions, ...recentConversations];
    const item = all.find((a) => a.id === value);
    if (item) item.action();
  }
</script>

<Dialog.Root bind:open={$commandPaletteOpen}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 bg-black/50 z-[100]" />
    <Dialog.Content
      class="fixed top-[20%] left-1/2 -translate-x-1/2 w-full max-w-lg bg-bg-surface border border-border-default rounded-xl shadow-2xl z-[101] overflow-hidden"
    >
      <Command.Root shouldFilter={true}>
        <div class="flex items-center gap-2 px-4 border-b border-border-default">
          <Search size={16} class="text-text-muted shrink-0" />
          <Command.Input
            placeholder="Type a command or search..."
            class="w-full py-3 bg-transparent border-none text-text-primary text-sm placeholder:text-text-faint focus:outline-none"
          />
        </div>

        <Command.List class="max-h-[300px] overflow-y-auto p-2">
          <Command.Empty class="py-6 text-center text-sm text-text-muted">
            No results found.
          </Command.Empty>

          <Command.Group>
            <Command.GroupHeading class="px-2 py-1.5 text-xs text-text-muted font-medium">
              Actions
            </Command.GroupHeading>
            <Command.GroupItems>
              {#each actions as item (item.id)}
                <Command.Item
                  value={item.id}
                  onSelect={() => handleSelect(item.id)}
                  class="flex items-center gap-3 px-3 py-2 rounded-md text-sm text-text-secondary cursor-pointer transition-colors duration-[--transition-fast] data-[highlighted]:bg-bg-overlay data-[highlighted]:text-text-primary"
                >
                  <item.icon size={16} class="text-text-muted shrink-0" />
                  {item.label}
                </Command.Item>
              {/each}
            </Command.GroupItems>
          </Command.Group>

          {#if recentConversations.length > 0}
            <Command.Group>
              <Command.GroupHeading class="px-2 py-1.5 text-xs text-text-muted font-medium mt-2">
                Recent Conversations
              </Command.GroupHeading>
              <Command.GroupItems>
                {#each recentConversations as item (item.id)}
                  <Command.Item
                    value={item.id}
                    onSelect={() => handleSelect(item.id)}
                    class="flex items-center gap-3 px-3 py-2 rounded-md text-sm text-text-secondary cursor-pointer transition-colors duration-[--transition-fast] data-[highlighted]:bg-bg-overlay data-[highlighted]:text-text-primary"
                  >
                    <item.icon size={16} class="text-text-muted shrink-0" />
                    <span class="truncate">{item.label}</span>
                  </Command.Item>
                {/each}
              </Command.GroupItems>
            </Command.Group>
          {/if}
        </Command.List>

        <div class="flex items-center justify-between px-4 py-2 border-t border-border-default text-xs text-text-faint">
          <span>Navigate with arrow keys</span>
          <span>
            <kbd class="px-1.5 py-0.5 bg-bg-overlay rounded text-text-muted">Esc</kbd> to close
          </span>
        </div>
      </Command.Root>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
