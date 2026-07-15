<script lang="ts">
	import { app, getEditingBlock, pushUndo } from '$lib/state.svelte';
	import { send } from '$lib/ipc';
	import type { Block } from '$lib/types';
	import X from '@lucide/svelte/icons/x';
	import { BLOCK_DOCS, hasVars } from './block-settings/shared';
	import HttpRequestSettings from './block-settings/HttpRequestSettings.svelte';
	import ParseSettings from './block-settings/ParseSettings.svelte';
	import KeyCheckSettings from './block-settings/KeyCheckSettings.svelte';
	import FunctionSettings from './block-settings/FunctionSettings.svelte';
	import ControlSettings from './block-settings/ControlSettings.svelte';
	import NetworkSettings from './block-settings/NetworkSettings.svelte';
	import BypassSettings from './block-settings/BypassSettings.svelte';
	import BrowserSettings from './block-settings/BrowserSettings.svelte';
	import JwtTokenSettings from './block-settings/JwtTokenSettings.svelte';
	import HeaderSpoofSettings from './block-settings/HeaderSpoofSettings.svelte';
	import AdvancedSettings from './block-settings/AdvancedSettings.svelte';

	let block = $derived(getEditingBlock());
	let open = $derived(app.editingBlockId !== null);

	function close() {
		app.editingBlockId = null;
	}

	function updateBlock(updates: Partial<Block>) {
		if (!block) return;
		pushUndo();
		const updated = { ...block, ...updates };
		send('update_block', updated);
	}

	function updateSettings(key: string, value: unknown) {
		if (!block) return;
		pushUndo();
		const updated = { ...block, settings: { ...block.settings, [key]: value } };
		send('update_block', updated);
	}

	const HTTP_TYPES = ['HttpRequest'];
	const PARSE_TYPES = ['ParseLR', 'ParseJSON', 'ParseRegex', 'ParseCSS', 'ParseXPath', 'ParseCookie', 'LambdaParser', 'Parse'];
	const KEYCHECK_TYPES = ['KeyCheck'];
	const FUNCTION_TYPES = ['StringFunction', 'Translate', 'ListFunction', 'CryptoFunction', 'ConversionFunction', 'ByteArray', 'Constants', 'Dictionary', 'FloatFunction', 'IntegerFunction', 'TimeFunction', 'GenerateGUID', 'PhoneCountry', 'FileSystem'];
	const CONTROL_TYPES = ['IfElse', 'Loop', 'Delay', 'Script', 'Log', 'SetVariable', 'ClearCookies'];
	const NETWORK_TYPES = ['Webhook', 'WebSocket', 'TcpRequest', 'UdpRequest', 'FtpRequest', 'SshRequest', 'ImapRequest', 'SmtpRequest', 'PopRequest'];
	const BYPASS_TYPES = ['CaptchaSolver', 'CloudflareBypass', 'LaravelCsrf'];
	const BROWSER_TYPES = ['BrowserOpen', 'NavigateTo', 'ClickElement', 'TypeText', 'WaitForElement', 'GetElementText', 'GetDom', 'Screenshot', 'ExecuteJs'];
	const ADVANCED_TYPES = ['DateFunction', 'CaseSwitch', 'CookieContainer', 'RandomUserAgent', 'OcrCaptcha', 'RecaptchaInvisible', 'XacfSensor', 'RandomData', 'DataDomeSensor', 'Plugin', 'Group', 'AkamaiV3Sensor'];
</script>

{#snippet embedBadge(val: string | undefined)}
	{#if hasVars(val)}
		<span class="absolute top-0.5 right-1 text-[8px] uppercase tracking-wider font-semibold text-primary/80 bg-primary/10 px-1 py-px rounded select-none pointer-events-none z-10">embed</span>
	{/if}
{/snippet}

{#if open && block}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="shrink-0 bg-surface border-l border-border flex flex-col slide-in-right overflow-hidden"
		style="width: {app.rightPanelWidth}px"
		onclick={(e) => e.stopPropagation()}
	>
		<!-- Header -->
		<div class="px-3 py-2 border-b border-border shrink-0 panel-raised flex items-center justify-between">
			<div class="flex items-center gap-2 min-w-0">
				<span class="text-sm font-medium text-foreground truncate">{block.label}</span>
				<span class="text-[9px] uppercase tracking-wider text-muted-foreground bg-background px-1.5 py-px rounded border border-border shrink-0">{block.block_type === 'Plugin' && block.settings?.plugin_block_type ? block.settings.plugin_block_type : block.block_type}</span>
			</div>
			<div class="flex items-center gap-1 shrink-0 ml-2">
				<button
					class="flex items-center gap-0.5 px-1.5 py-0.5 rounded hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
					onclick={() => { app.blockDocsInitialType = block.settings.type; app.showBlockDocs = true; }}
					title="Documentation (F1)"
				>
					<span class="text-[10px] font-semibold">?</span>
					<span class="text-[9px]">Docs</span>
				</button>
				<button
					class="p-0.5 rounded hover:bg-secondary text-muted-foreground hover:text-foreground"
					onclick={close}
					title="Close"
				>
					<X size={14} />
				</button>
			</div>
		</div>

		<!-- Block options: disabled / safe mode -->
		<div class="flex gap-3 px-3 py-1.5 border-b border-border bg-surface shrink-0">
			<label class="flex items-center gap-1 text-[10px] text-muted-foreground">
				<input type="checkbox" checked={block.disabled} onchange={() => updateBlock({ disabled: !block!.disabled })} class="skeu-checkbox" />
				Disabled
			</label>
			<label class="flex items-center gap-1 text-[10px] text-muted-foreground">
				<input type="checkbox" checked={block.safe_mode} onchange={() => updateBlock({ safe_mode: !block!.safe_mode })} class="skeu-checkbox" />
				Safe Mode
			</label>
		</div>

		<!-- Settings body -->
		<div class="flex-1 overflow-y-auto p-2 space-y-1.5 panel-inset">

			<!-- Block documentation summary -->
			{#if BLOCK_DOCS[block.settings.type]}
				<p class="text-[10px] text-muted-foreground/70 italic leading-snug pb-1 border-b border-border/50 mb-1">{BLOCK_DOCS[block.settings.type].summary}</p>
			{/if}

			{#if HTTP_TYPES.includes(block.settings.type)}
				<HttpRequestSettings {block} {updateSettings} {embedBadge} />
			{:else if PARSE_TYPES.includes(block.settings.type)}
				<ParseSettings {block} {updateSettings} {embedBadge} />
			{:else if KEYCHECK_TYPES.includes(block.settings.type)}
				<KeyCheckSettings {block} {updateSettings} {embedBadge} />
			{:else if FUNCTION_TYPES.includes(block.settings.type)}
				<FunctionSettings {block} {updateSettings} />
			{:else if CONTROL_TYPES.includes(block.settings.type)}
				<ControlSettings {block} {updateSettings} {embedBadge} />
			{:else if NETWORK_TYPES.includes(block.settings.type)}
				<NetworkSettings {block} {updateSettings} {embedBadge} />
			{:else if BYPASS_TYPES.includes(block.settings.type)}
				<BypassSettings {block} {updateSettings} {embedBadge} />
			{:else if block.settings.type === 'JwtToken'}
				<JwtTokenSettings {block} {updateSettings} {embedBadge} />
			{:else if block.settings.type === 'HeaderSpoof'}
				<HeaderSpoofSettings {block} {updateSettings} {embedBadge} />
			{:else if BROWSER_TYPES.includes(block.settings.type)}
				<BrowserSettings {block} {updateSettings} {embedBadge} />
			{:else if ADVANCED_TYPES.includes(block.settings.type)}
				<AdvancedSettings {block} {updateSettings} {embedBadge} />
			{:else}
				<div class="text-[11px] text-muted-foreground">
					Settings editor for {block.block_type} blocks
				</div>
			{/if}
		</div>
	</div>
{/if}
