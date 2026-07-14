<script lang="ts">
	import { app } from '$lib/state.svelte';
	import { send, savePipeline, saveSettings } from '$lib/ipc';
	import ExternalLink from '@lucide/svelte/icons/external-link';
	import FolderOpen from '@lucide/svelte/icons/folder-open';

	let { searchQuery, shouldShowSetting }: {
		searchQuery: string;
		shouldShowSetting: (section: string, label: string) => boolean;
	} = $props();
</script>

{#if searchQuery}<div class="text-[10px] uppercase tracking-widest text-muted-foreground/70 font-semibold mb-2 mt-2">Paths</div>{/if}

{#if shouldShowSetting('paths', 'Collections Folder')}
	<div class="py-1.5">
		<div class="flex items-center justify-between">
			<div>
				<span class="text-[11px] text-muted-foreground">Collections folder</span>
				<p class="text-[9px] text-muted-foreground/60">Folder containing .rfx configs for quick access</p>
			</div>
			<button class="skeu-btn text-[10px]" onclick={() => send('browse_folder', { field: 'collections' })}>Browse</button>
		</div>
		<input
			type="text"
			bind:value={app.collectionsPath}
			class="w-full skeu-input text-[10px] font-mono mt-1"
			placeholder="Not set"
		/>
	</div>
{/if}

{#if shouldShowSetting('paths', 'Wordlist Directory')}
	<div class="py-1.5">
		<div class="flex items-center justify-between">
			<div>
				<span class="text-[11px] text-muted-foreground">Default wordlist directory</span>
				<p class="text-[9px] text-muted-foreground/60">Start browsing wordlists from this folder</p>
			</div>
			<button class="skeu-btn text-[10px]" onclick={() => send('browse_folder', { field: 'wordlist_dir' })}>Browse</button>
		</div>
		<input
			type="text"
			bind:value={app.defaultWordlistPath}
			class="w-full skeu-input text-[10px] font-mono mt-1"
			placeholder="Not set"
		/>
	</div>
{/if}

{#if shouldShowSetting('paths', 'Proxy Directory')}
	<div class="py-1.5">
		<div class="flex items-center justify-between">
			<div>
				<span class="text-[11px] text-muted-foreground">Default proxy directory</span>
				<p class="text-[9px] text-muted-foreground/60">Start browsing proxy files from this folder</p>
			</div>
			<button class="skeu-btn text-[10px]" onclick={() => send('browse_folder', { field: 'proxy_dir' })}>Browse</button>
		</div>
		<input
			type="text"
			bind:value={app.defaultProxyPath}
			class="w-full skeu-input text-[10px] font-mono mt-1"
			placeholder="Not set"
		/>
	</div>
{/if}

{#if shouldShowSetting('paths', 'Chrome Executable')}
	<div class="py-1.5">
		<div class="flex items-center justify-between gap-1">
			<div class="flex-1 min-w-0">
				<span class="text-[11px] text-muted-foreground">Chrome / Chromium executable</span>
				<p class="text-[9px] text-muted-foreground/60">Path to chrome.exe (or chromium). Leave blank for auto-detection. Use this when Chrome is installed but not found automatically.</p>
			</div>
			<div class="flex items-center gap-1 shrink-0">
				<button
					class="skeu-btn text-[10px] flex items-center gap-1"
					title="Open Chrome download page"
					onclick={() => send('open_url', { url: 'https://www.google.com/chrome/' })}
				>
					<ExternalLink size={11} />
					Download
				</button>
				<button
					class="skeu-btn text-[10px]"
					onclick={() => send('browse_file', { field: 'chrome_exe' })}
				>
					Browse
				</button>
			</div>
		</div>
		<input
			type="text"
			value={(app.config as any).chrome_executable_path ?? ''}
			oninput={(e) => {
				const val = (e.target as HTMLInputElement).value;
				(app.config as any).chrome_executable_path = val;
			}}
			onblur={() => {
				saveSettings();
			}}
			class="w-full skeu-input text-[10px] font-mono mt-1"
			placeholder="Auto-detect (leave blank)"
		/>
	</div>
{/if}

{#if shouldShowSetting('paths', 'Configs Directory')}
	<div class="py-1.5">
		<div class="flex items-center justify-between">
			<div>
				<span class="text-[11px] text-muted-foreground">Default configs directory</span>
				<p class="text-[9px] text-muted-foreground/60">Start browsing/saving config files from this folder</p>
			</div>
			<button class="skeu-btn text-[10px]" onclick={() => send('browse_folder', { field: 'configs' })}>Browse</button>
		</div>
		<input
			type="text"
			bind:value={app.configsPath}
			class="w-full skeu-input text-[10px] font-mono mt-1"
			placeholder="Not set"
		/>
	</div>
{/if}

{#if shouldShowSetting('paths', 'Results Directory')}
	<div class="py-1.5">
		<div class="flex items-center justify-between">
			<div>
				<span class="text-[11px] text-muted-foreground">Default results directory</span>
				<p class="text-[9px] text-muted-foreground/60">Default folder for output/hit files</p>
			</div>
			<button class="skeu-btn text-[10px]" onclick={() => send('browse_folder', { field: 'results' })}>Browse</button>
		</div>
		<input
			type="text"
			bind:value={app.resultsPath}
			class="w-full skeu-input text-[10px] font-mono mt-1"
			placeholder="Not set"
		/>
	</div>
{/if}

{#if !searchQuery}<div class="my-2 groove-h h-px"></div>{/if}
