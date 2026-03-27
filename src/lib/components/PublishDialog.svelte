<script lang="ts">
  import { Dialog } from "bits-ui";
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { validateHfToken, getHfToken, saveHfToken, publishToHuggingface } from "$lib/api";
  import type { ExportFormat, PublishConfig, PublishResult, PublishProgress, WhoamiResponse, RedactConfig } from "$lib/types";
  import { setLoading, setMessage, setError, clearStatus } from "$lib/stores/statusStore";
  import { Upload, ExternalLink, Check, Loader2, AlertCircle, KeyRound } from "lucide-svelte";

  let {
    open = $bindable(false),
    format = "ChatML" as ExportFormat,
    projectPaths = [] as string[],
    defaultRepoName = "training-data",
  }: {
    open: boolean;
    format?: ExportFormat;
    projectPaths?: string[];
    defaultRepoName?: string;
  } = $props();

  // State machine: token → config → publishing → done → error
  let step = $state<"token" | "config" | "publishing" | "done" | "error">("token");
  let token = $state("");
  let tokenValidating = $state(false);
  let tokenError = $state<string | null>(null);
  let username = $state("");
  let orgs = $state<{ name: string }[]>([]);

  // Config
  let repoName = $state("");
  let namespace = $state("");
  let isPrivate = $state(false);
  let selectedFormat = $state<ExportFormat>("ChatML");
  let license = $state("mit");

  // Redaction
  let redactApiKeys = $state(true);
  let redactHomePaths = $state(true);
  let redactEmails = $state(false);
  let redactIpAddresses = $state(false);

  // Progress
  let progressStep = $state("");
  let progressDetail = $state("");

  // Result
  let result = $state<PublishResult | null>(null);
  let errorMsg = $state<string | null>(null);

  // When dialog opens, check for saved token
  $effect(() => {
    if (open) {
      repoName = defaultRepoName;
      selectedFormat = format;
      checkSavedToken();
    }
  });

  async function checkSavedToken() {
    step = "token";
    tokenError = null;
    try {
      const saved = await getHfToken();
      if (saved) {
        token = saved;
        await validateAndProceed();
      }
    } catch {
      // No saved token, show input
    }
  }

  async function validateAndProceed() {
    if (!token.trim()) {
      tokenError = "Please enter a token";
      return;
    }
    tokenValidating = true;
    tokenError = null;
    try {
      const whoami: WhoamiResponse = await validateHfToken(token);
      // Save token on successful validation (backend stores it, never crosses IPC again)
      await saveHfToken(token);
      username = whoami.name;
      orgs = whoami.orgs || [];
      namespace = username;
      step = "config";
    } catch (e) {
      tokenError = `${e}`;
      step = "token";
    } finally {
      tokenValidating = false;
    }
  }

  async function handlePublish() {
    step = "publishing";
    progressStep = "Starting...";
    progressDetail = "";
    result = null;
    errorMsg = null;

    const unlisten = await listen<PublishProgress>("publish-progress", (event) => {
      const p = event.payload;
      switch (p.step) {
        case "ValidatingToken": progressStep = "Validating token..."; break;
        case "ExportingData": progressStep = "Exporting conversations..."; break;
        case "CreatingRepo": progressStep = "Creating repository..."; break;
        case "GeneratingCard": progressStep = "Generating dataset card..."; break;
        case "Uploading": progressStep = `Uploading files (${p.current}/${p.total})...`; break;
        case "Committing": progressStep = "Committing to HuggingFace..."; break;
        case "Done": progressStep = "Done!"; break;
      }
      setLoading(progressStep);
    });

    try {
      const config: PublishConfig = {
        repo_name: repoName,
        namespace: namespace || null,
        private: isPrivate,
        license,
        format: selectedFormat,
        project_paths: projectPaths,
        redact_config: {
          redact_api_keys: redactApiKeys,
          redact_home_paths: redactHomePaths,
          redact_emails: redactEmails,
          redact_ip_addresses: redactIpAddresses,
          custom_rules: [],
        },
      };

      result = await publishToHuggingface(config);
      step = "done";
      setMessage("Published to HuggingFace!");
    } catch (e) {
      errorMsg = `${e}`;
      step = "error";
      setError(`Publish failed: ${e}`);
    } finally {
      unlisten();
      clearStatus();
    }
  }

  function handleClose() {
    open = false;
    step = "token";
    token = "";
    result = null;
    errorMsg = null;
  }

  const inputClass = "w-full px-3 py-2 bg-bg-elevated border border-border-default rounded-md text-sm text-text-primary placeholder:text-text-faint focus:outline-none focus:border-accent";
  const labelClass = "block text-xs font-medium text-text-secondary mb-1";
  const btnPrimary = "flex items-center justify-center gap-2 w-full px-4 py-2 bg-accent border-none rounded-md text-sm font-medium text-text-primary cursor-pointer transition-colors hover:bg-accent-hover disabled:opacity-50 disabled:cursor-not-allowed";
  const btnSecondary = "flex items-center justify-center gap-2 w-full px-4 py-2 bg-bg-surface border border-border-default rounded-md text-sm text-text-secondary cursor-pointer transition-colors hover:bg-bg-overlay hover:text-text-primary";
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 bg-black/50 z-[100]" />
    <Dialog.Content
      class="fixed top-[15%] left-1/2 -translate-x-1/2 w-full max-w-md bg-bg-surface border border-border-default rounded-xl shadow-2xl z-[101] overflow-hidden"
    >
      <div class="px-6 py-4 border-b border-border-default">
        <h2 class="m-0 text-lg font-semibold text-text-primary flex items-center gap-2">
          <Upload size={18} />
          Publish to HuggingFace
        </h2>
      </div>

      <div class="p-6">
        {#if step === "token"}
          <!-- Token Entry -->
          <div class="space-y-4">
            <p class="text-sm text-text-secondary m-0">
              Enter your HuggingFace token with <strong>write</strong> access.
            </p>
            <div>
              <label class={labelClass} for="hf-token">
                <KeyRound size={12} class="inline mr-1" />API Token
              </label>
              <input
                id="hf-token"
                type="password"
                bind:value={token}
                placeholder="hf_..."
                class={inputClass}
                onkeydown={(e) => { if (e.key === 'Enter') validateAndProceed(); }}
              />
            </div>
            {#if tokenError}
              <div class="flex items-start gap-2 p-3 rounded-md bg-danger/10 border border-danger/30 text-danger text-xs">
                <AlertCircle size={14} class="shrink-0 mt-0.5" />
                {tokenError}
              </div>
            {/if}
            <button class={btnPrimary} onclick={validateAndProceed} disabled={tokenValidating || !token.trim()}>
              {#if tokenValidating}
                <Loader2 size={14} class="animate-spin" /> Validating...
              {:else}
                Validate Token
              {/if}
            </button>
            <p class="text-[11px] text-text-faint m-0">
              <!-- svelte-ignore a11y_invalid_attribute -->
              <a href="#" class="text-accent-hover hover:underline" onclick={(e) => { e.preventDefault(); openUrl("https://huggingface.co/settings/tokens"); }}>
                Create a token at huggingface.co/settings/tokens
              </a>
            </p>
          </div>

        {:else if step === "config"}
          <!-- Publish Config -->
          <div class="space-y-3">
            <p class="text-sm text-text-secondary m-0">
              Signed in as <strong>{username}</strong>
            </p>

            <div>
              <label class={labelClass} for="repo-name">Repository Name</label>
              <input
                id="repo-name"
                type="text"
                bind:value={repoName}
                placeholder="my-training-data"
                class={inputClass}
              />
            </div>

            <div>
              <label class={labelClass} for="namespace">Namespace</label>
              <select id="namespace" bind:value={namespace} class={inputClass}>
                <option value={username}>{username} (personal)</option>
                {#each orgs as org}
                  <option value={org.name}>{org.name} (org)</option>
                {/each}
              </select>
            </div>

            <div>
              <label class={labelClass} for="format-select">Format</label>
              <select id="format-select" bind:value={selectedFormat} class={inputClass}>
                <option value="ChatML">ChatML (OpenAI)</option>
                <option value="ChatMLTools">ChatML + Tools</option>
                <option value="ShareGPT">ShareGPT</option>
                <option value="Alpaca">Alpaca</option>
              </select>
            </div>

            <div>
              <label class={labelClass} for="license-select">License</label>
              <select id="license-select" bind:value={license} class={inputClass}>
                <option value="mit">MIT</option>
                <option value="apache-2.0">Apache 2.0</option>
                <option value="cc-by-4.0">CC BY 4.0</option>
                <option value="cc-by-sa-4.0">CC BY-SA 4.0</option>
                <option value="cc-by-nc-4.0">CC BY-NC 4.0</option>
              </select>
            </div>

            <div class="flex items-center gap-2">
              <input id="private-toggle" type="checkbox" bind:checked={isPrivate} class="accent-accent-hover" />
              <label for="private-toggle" class="text-sm text-text-secondary cursor-pointer">Private repository</label>
            </div>

            <!-- Sanitization -->
            <div class="pt-2 border-t border-border-default">
              <div class={labelClass}>Sanitization</div>
              <div class="space-y-1.5 mt-1">
                <label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
                  <input type="checkbox" bind:checked={redactApiKeys} class="accent-accent-hover" />
                  Redact API keys & secrets
                </label>
                <label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
                  <input type="checkbox" bind:checked={redactHomePaths} class="accent-accent-hover" />
                  Replace home directory paths
                </label>
                <label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
                  <input type="checkbox" bind:checked={redactEmails} class="accent-accent-hover" />
                  Redact email addresses
                </label>
                <label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
                  <input type="checkbox" bind:checked={redactIpAddresses} class="accent-accent-hover" />
                  Redact IP addresses
                </label>
              </div>
            </div>

            <button class={btnPrimary} onclick={handlePublish} disabled={!repoName.trim()}>
              <Upload size={14} /> Publish
            </button>
            <button class={btnSecondary} onclick={handleClose}>Cancel</button>
          </div>

        {:else if step === "publishing"}
          <!-- Progress -->
          <div class="space-y-4 text-center py-4">
            <Loader2 size={32} class="animate-spin text-accent-hover mx-auto" />
            <p class="text-sm text-text-primary font-medium m-0">{progressStep}</p>
            {#if progressDetail}
              <p class="text-xs text-text-muted m-0">{progressDetail}</p>
            {/if}
          </div>

        {:else if step === "done" && result}
          <!-- Success -->
          <div class="space-y-4 text-center py-4">
            <div class="w-12 h-12 rounded-full bg-success/20 flex items-center justify-center mx-auto">
              <Check size={24} class="text-success" />
            </div>
            <p class="text-sm text-text-primary font-medium m-0">Published successfully!</p>
            <p class="text-xs text-text-muted m-0">{result.files_uploaded} files uploaded</p>
            <button
              class={btnPrimary}
              onclick={() => openUrl(result!.repo_url)}
            >
              <ExternalLink size={14} /> Open on HuggingFace
            </button>
            <button class={btnSecondary} onclick={handleClose}>Close</button>
          </div>

        {:else if step === "error"}
          <!-- Error -->
          <div class="space-y-4 py-4">
            <div class="flex items-start gap-2 p-3 rounded-md bg-danger/10 border border-danger/30 text-danger text-sm">
              <AlertCircle size={16} class="shrink-0 mt-0.5" />
              {errorMsg}
            </div>
            <button class={btnPrimary} onclick={() => { step = "config"; }}>
              Try Again
            </button>
            <button class={btnSecondary} onclick={handleClose}>Close</button>
          </div>
        {/if}
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
