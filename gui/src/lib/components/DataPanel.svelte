<script lang="ts">
	import { app } from '$lib/state.svelte';
	import { send } from '$lib/ipc';
	import { syncPipelineToBackend } from '$lib/state/tabs';
	import { onDestroy } from 'svelte';
	import SkeuSelect from './SkeuSelect.svelte';
	import type { ProxySource } from '$lib/types';
	import Plus from '@lucide/svelte/icons/plus';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import Database from '@lucide/svelte/icons/database';
	import FileText from '@lucide/svelte/icons/file-text';
	import Shield from '@lucide/svelte/icons/shield';
	import Globe from '@lucide/svelte/icons/globe';
	import Link from '@lucide/svelte/icons/link';
	import ListIcon from '@lucide/svelte/icons/list';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import Download from '@lucide/svelte/icons/download';

	function browseProxies() {
		send('browse_file', { field: 'proxies' });
	}

	// Slice management
	function updateSlices(raw: string) {
		app.pipeline.data_settings.slices = raw.split(',').map(s => s.trim()).filter(s => s.length > 0);
	}

	// Wordlist type presets (OB2-style)
	const WORDLIST_PRESETS = [
		{ value: 'Combolist', label: 'Combolist (USER:PASS)', sep: ':', slices: ['USER', 'PASS'] },
		{ value: 'CombolistEmail', label: 'Combolist (EMAIL:PASS)', sep: ':', slices: ['EMAIL', 'PASS'] },
		{ value: 'CombolistLogin', label: 'Combolist (LOGIN:PASS)', sep: ':', slices: ['LOGIN', 'PASS'] },
		{ value: 'Emails', label: 'Email list', sep: ':', slices: ['EMAIL'] },
		{ value: 'Tokens', label: 'Token list', sep: ':', slices: ['TOKEN'] },
		{ value: 'Custom', label: 'Custom', sep: ':', slices: [] },
	];

	function applyPreset(preset: typeof WORDLIST_PRESETS[0]) {
		app.pipeline.data_settings.wordlist_type = preset.value;
		app.pipeline.data_settings.separator = preset.sep;
		if (preset.slices.length > 0) {
			app.pipeline.data_settings.slices = [...preset.slices];
		}
	}

	// Proxy ban settings
	function updateBanDuration(val: string) {
		const num = parseInt(val);
		if (!isNaN(num) && num >= 0) {
			app.pipeline.proxy_settings.ban_duration_secs = num;
		}
	}

	function updateMaxRetries(val: string) {
		const num = parseInt(val);
		if (!isNaN(num) && num >= 0) {
			app.pipeline.proxy_settings.max_retries_before_ban = num;
		}
	}

	function updateConcurrentPerProxy(val: string) {
		const num = parseInt(val);
		if (!isNaN(num) && num >= 0) {
			app.pipeline.runner_settings.concurrent_per_proxy = num;
		}
	}

	// Proxy source management
	function addProxySource(type: 'File' | 'Url' | 'Inline') {
		app.pipeline.proxy_settings.proxy_sources = [
			...app.pipeline.proxy_settings.proxy_sources,
			{ source_type: type, value: '', refresh_interval_secs: 0, default_proxy_type: undefined },
		];
		// If mode is None, auto-switch to Rotate so the source is actually used.
		if (app.pipeline.proxy_settings.proxy_mode === 'None') {
			app.pipeline.proxy_settings.proxy_mode = 'Rotate';
			console.log('[DataPanel] addProxySource: proxy_mode was None — auto-switched to Rotate');
		}
	}

	function removeProxySource(index: number) {
		app.pipeline.proxy_settings.proxy_sources = app.pipeline.proxy_settings.proxy_sources.filter((_, i) => i !== index);
	}

	let encryptedProxyUri = $state('');
	let encryptedProxyNotice = $state('');

	function importEncryptedProxy() {
		const uri = encryptedProxyUri.trim();
		const scheme = uri.split('://', 1)[0]?.toLowerCase();
		if (!['vmess', 'vless', 'trojan'].includes(scheme)) {
			encryptedProxyNotice = 'Paste a complete vmess://, vless://, or trojan:// URI.';
			return;
		}
		if (app.pipeline.proxy_settings.proxy_sources.some(source => source.source_type === 'Inline' && source.value.split(/\r?\n/).includes(uri))) {
			encryptedProxyNotice = 'This encrypted proxy is already in the active pipeline sources.';
			return;
		}
		app.pipeline.proxy_settings.proxy_sources = [
			...app.pipeline.proxy_settings.proxy_sources,
			{ source_type: 'Inline', value: uri, refresh_interval_secs: 0, default_proxy_type: undefined },
		];
		if (app.pipeline.proxy_settings.proxy_mode === 'None') app.pipeline.proxy_settings.proxy_mode = 'Rotate';
		encryptedProxyUri = '';
		encryptedProxyNotice = 'Added as a managed Xray source. Use Debug → Proxy to test this URI before starting a job.';
		syncPipelineToBackend();
	}

	// Proxy group management
	let newGroupName = $state('');

	function addProxyGroup() {
		const name = newGroupName.trim();
		if (!name) return;
		if (app.pipeline.proxy_settings.proxy_groups.some(g => g.name === name)) return;
		app.pipeline.proxy_settings.proxy_groups = [
			...app.pipeline.proxy_settings.proxy_groups,
			{ name, mode: 'Rotate', sources: [], cpm_per_proxy: 0 },
		];
		newGroupName = '';
		syncPipelineToBackend();
	}

	function removeProxyGroup(index: number) {
		const removed = app.pipeline.proxy_settings.proxy_groups[index];
		app.pipeline.proxy_settings.proxy_groups = app.pipeline.proxy_settings.proxy_groups.filter((_, i) => i !== index);
		if (app.pipeline.proxy_settings.active_group === removed?.name) {
			app.pipeline.proxy_settings.active_group = '';
		}
		syncPipelineToBackend();
	}

	function addGroupSource(groupIdx: number, type: 'File' | 'Url' | 'Inline') {
		const groups = [...app.pipeline.proxy_settings.proxy_groups];
		groups[groupIdx] = { ...groups[groupIdx], sources: [...groups[groupIdx].sources, { source_type: type, value: '', refresh_interval_secs: 0, default_proxy_type: undefined }] };
		app.pipeline.proxy_settings.proxy_groups = groups;
		syncPipelineToBackend();
	}

	function removeGroupSource(groupIdx: number, sourceIdx: number) {
		const groups = [...app.pipeline.proxy_settings.proxy_groups];
		groups[groupIdx] = { ...groups[groupIdx], sources: groups[groupIdx].sources.filter((_, i) => i !== sourceIdx) };
		app.pipeline.proxy_settings.proxy_groups = groups;
		syncPipelineToBackend();
	}

	onDestroy(() => {});

	// ── Hits Database ──────────────────────────────────────────────────────
	let selectedHitsJobId = $state<string | null>(null);

	// Derived: hits array for the currently selected job
	const hitsForJob = $derived(
		selectedHitsJobId ? (app.jobHitsDb[selectedHitsJobId] ?? []) : []
	);

	// If the selected job gets deleted, clear selection automatically
	$effect(() => {
		if (selectedHitsJobId && !app.jobs.find((j: any) => j.id === selectedHitsJobId)) {
			selectedHitsJobId = null;
		}
	});

	// Respond to external navigation requests (e.g. clicking "View Hits" in JobMonitor)
	$effect(() => {
		const id = app.hitsDbJobId;
		if (id && id !== selectedHitsJobId) {
			selectHitsJob(id);
			app.hitsDbJobId = null; // consume the signal
		}
	});

	function selectHitsJob(id: string) {
		selectedHitsJobId = id || null;
		if (selectedHitsJobId) {
			// Fetch latest snapshot from backend on selection
			send('get_job_hits', { id: selectedHitsJobId });
		}
	}

	function refreshHits() {
		if (selectedHitsJobId) {
			send('get_job_hits', { id: selectedHitsJobId });
		}
	}

	function exportHitsTxt() {
		if (!hitsForJob.length) return;
		const lines = hitsForJob.map(h => h.data_line).join('\n');
		const blob = new Blob([lines], { type: 'text/plain' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a'); a.href = url; a.download = 'hits.txt'; a.click();
		URL.revokeObjectURL(url);
	}

	function exportHitsCsv() {
		if (!hitsForJob.length) return;
		const cols = [...new Set(hitsForJob.flatMap(h => Object.keys(h.captures ?? {})))];
		const header = ['data_line', 'proxy', ...cols].join(',');
		const rows = hitsForJob.map(h =>
			[h.data_line, h.proxy ?? '', ...cols.map(c => h.captures?.[c] ?? '')].map(v => `"${String(v).replace(/"/g, '""')}"`).join(',')
		);
		const csv = [header, ...rows].join('\n');
		const blob = new Blob([csv], { type: 'text/csv' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a'); a.href = url; a.download = 'hits.csv'; a.click();
		URL.revokeObjectURL(url);
	}
</script>

<div class="flex flex-col h-full bg-surface p-2 space-y-2.5 overflow-y-auto">
	<!-- Data Settings -->
	<div>
		<div class="text-[10px] uppercase tracking-wider text-muted-foreground mb-1.5 flex items-center gap-1.5">
			<Database size={11} />
			Data Settings
		</div>
		<div class="space-y-1.5">
			<div>
				<label class="text-[10px] text-muted-foreground">Wordlist Type</label>
				<SkeuSelect value={app.pipeline.data_settings.wordlist_type}
					onValueChange={(v) => {
						const preset = WORDLIST_PRESETS.find(p => p.value === v);
						if (preset) applyPreset(preset);
					}}
					options={WORDLIST_PRESETS.map(p => ({ value: p.value, label: p.label }))}
					class="w-full text-[11px] mt-0.5"
				/>
			</div>
			<div class="flex gap-2">
				<div class="w-16">
					<label class="text-[10px] text-muted-foreground">Separator</label>
					<input type="text" value={app.pipeline.data_settings.separator}
						class="w-full skeu-input text-[11px] text-center font-mono mt-0.5"
						oninput={(e) => { app.pipeline.data_settings.separator = (e.target as HTMLInputElement).value; }} />
				</div>
				<div class="flex-1">
					<label class="text-[10px] text-muted-foreground">Variable slices</label>
					<input type="text" value={app.pipeline.data_settings.slices.join(', ')}
						class="w-full skeu-input text-[11px] font-mono mt-0.5"
						placeholder="USER, PASS"
						oninput={(e) => updateSlices((e.target as HTMLInputElement).value)} />
					<p class="text-[9px] text-muted-foreground mt-0.5">Comma-separated. Available as <code class="text-foreground/70">input.USER</code>, <code class="text-foreground/70">input.PASS</code></p>
				</div>
			</div>
		</div>
	</div>

	<!-- Wordlist path is set per-job in the Jobs tab. Only data format is configured here. -->

	<!-- Proxy Settings -->
	<div>
		<div class="text-[10px] uppercase tracking-wider text-muted-foreground mb-1.5 flex items-center gap-1.5">
			<Shield size={11} />
			Proxy Settings
		</div>
		<div class="space-y-1.5">
			<SkeuSelect value={app.pipeline.proxy_settings.proxy_mode}
				onValueChange={(v) => { app.pipeline.proxy_settings.proxy_mode = v as any; }}
				options={[
					{value:'None',label:'No Proxies'},
					{value:'Rotate',label:'Random proxy per request'},
					{value:'Sticky',label:'1 proxy per account check'},
					{value:'CpmLimited',label:'CPM limit per proxy'},
				]}
				class="w-full text-[11px]"
			/>
			{#if app.pipeline.proxy_settings.proxy_mode === 'None'}
				<button
					class="mt-1.5 w-full rounded border border-primary/40 bg-primary/5 px-2 py-1.5 text-left"
					onclick={() => { app.pipeline.proxy_settings.proxy_mode = 'Rotate'; encryptedProxyNotice = 'Proxy rotation enabled. Paste an encrypted URI below to import it.'; }}
				>
					<span class="block text-[10px] font-medium text-foreground">Import VMess, VLESS, or Trojan</span>
					<span class="block text-[9px] text-muted-foreground/70 mt-0.5">Enable managed encrypted-proxy sources and test one in Debug before running a job.</span>
				</button>
			{/if}

			<!-- Proxy Sources -->
			{#if app.pipeline.proxy_settings.proxy_mode !== 'None'}
				<div class="space-y-1">
				<div class="rounded border border-border bg-background p-2 space-y-1.5">
					<div class="flex items-center justify-between gap-2">
						<div>
							<p class="text-[10px] font-medium text-foreground">Encrypted proxy import</p>
							<p class="text-[9px] text-muted-foreground/70">VMess, VLESS, and Trojan run through bundled Xray Core as local SOCKS5.</p>
						</div>
						<span class="text-[8px] font-mono text-primary shrink-0">MANAGED</span>
					</div>
					<div class="flex gap-1">
						<input
							type="text"
							bind:value={encryptedProxyUri}
							placeholder="vmess:// · vless:// · trojan://"
							class="skeu-input flex-1 text-[10px] font-mono"
							onkeydown={(e) => { if (e.key === 'Enter') importEncryptedProxy(); }}
						/>
						<button class="skeu-btn text-[9px] px-2" onclick={importEncryptedProxy}>Import</button>
					</div>
					<p class="text-[8px] leading-tight text-muted-foreground/60">Imported URIs are saved as Inline sources. For a single URI, copy it into Debug → Proxy and run F5 first.</p>
					{#if encryptedProxyNotice}<p class="text-[9px] leading-tight {encryptedProxyNotice.startsWith('Added') ? 'text-green' : 'text-orange'}">{encryptedProxyNotice}</p>{/if}
				</div>

					{#each app.pipeline.proxy_settings.proxy_sources as source, i}
						<div class="bg-background rounded p-1.5 border border-border space-y-1">
							<div class="flex items-center gap-1">
								{#if source.source_type === 'Url'}
									<Link size={10} class="text-muted-foreground shrink-0" />
								{:else if source.source_type === 'File'}
									<FileText size={10} class="text-muted-foreground shrink-0" />
								{:else}
									<ListIcon size={10} class="text-muted-foreground shrink-0" />
								{/if}
								<!-- Proxy type per source — overrides auto-detection for plain ip:port lines -->
								<SkeuSelect
									value={source.default_proxy_type ?? ''}
									onValueChange={(v) => {
										const srcs = [...app.pipeline.proxy_settings.proxy_sources];
										srcs[i] = { ...srcs[i], default_proxy_type: v || undefined };
										app.pipeline.proxy_settings.proxy_sources = srcs;
									}}
									options={[
										{ value: '', label: 'Auto' },
										{ value: 'Http', label: 'HTTP' },
										{ value: 'Https', label: 'HTTPS' },
										{ value: 'Socks4', label: 'SOCKS4' },
										{ value: 'Socks5', label: 'SOCKS5' },
										{ value: 'Shadowsocks', label: 'Shadowsocks' },
									]}
									class="text-[9px] w-[72px] shrink-0"
									title="Protocol type for plain ip:port lines"
								/>
								<input type="text"
									bind:value={source.value}
									placeholder={source.source_type === 'Url' ? 'https://domain.com/proxylist.txt' : source.source_type === 'File' ? '/path/to/proxies.txt' : 'ip:port per line...'}
									class="flex-1 skeu-input text-[10px] font-mono"
								/>
								{#if source.source_type === 'File'}
									<button class="skeu-btn text-[9px] px-1.5 py-0.5" onclick={browseProxies}>...</button>
								{/if}
								<button class="p-0.5 text-muted-foreground hover:text-red" onclick={() => removeProxySource(i)} title="Remove">
									<Trash2 size={10} />
								</button>
							</div>
							<p class="text-[8px] text-muted-foreground/40 pl-1 leading-tight">
								<span class="font-mono">ip:port</span> · <span class="font-mono">ip:port:user:pass</span> · <span class="font-mono">socks5://ip:port</span> · <span class="font-mono">socks5://user:pass@ip:port</span>
							</p>
							{#if source.source_type === 'Url'}
								<div class="flex items-center gap-1.5 pl-4">
									<RefreshCw size={9} class="text-muted-foreground/60" />
									<span class="text-[9px] text-muted-foreground/60">Refresh every</span>
									<input type="number" min="0"
										bind:value={source.refresh_interval_secs}
										class="w-14 skeu-input text-[9px] text-center"
										placeholder="0"
									/>
									<span class="text-[9px] text-muted-foreground/60">sec (0 = once)</span>
								</div>
							{/if}
						</div>
					{/each}
				</div>

				<div class="flex gap-1">
					<button class="skeu-btn text-[9px] flex items-center gap-1 flex-1" onclick={() => addProxySource('File')}>
						<FileText size={9} /> File
					</button>
					<button class="skeu-btn text-[9px] flex items-center gap-1 flex-1" onclick={() => addProxySource('Url')}>
						<Link size={9} /> URL
					</button>
					<button class="skeu-btn text-[9px] flex items-center gap-1 flex-1" onclick={() => addProxySource('Inline')}>
						<ListIcon size={9} /> Inline
					</button>
				</div>

				<!-- Quick add proxy file → adds to proxy sources list -->
				<div class="flex gap-1">
					<input type="text" bind:value={app.proxyPath} placeholder="Quick add proxy file..."
						class="flex-1 skeu-input text-[10px] font-mono" />
					<button class="skeu-btn text-[10px]" onclick={browseProxies}>Browse</button>
					<button
						class="skeu-btn text-[10px]"
						disabled={!app.proxyPath}
						title="Add path as File proxy source"
						onclick={() => {
							if (!app.proxyPath) return;
							const already = app.pipeline.proxy_settings.proxy_sources.some(s => s.source_type === 'File' && s.value === app.proxyPath);
							if (already) { console.log('[DataPanel] proxy source already added:', app.proxyPath); return; }
							app.pipeline.proxy_settings.proxy_sources = [
								...app.pipeline.proxy_settings.proxy_sources,
								{ source_type: 'File', value: app.proxyPath, refresh_interval_secs: 0, default_proxy_type: undefined },
							];
							// Auto-switch mode from None → Rotate so the pool is actually used.
							// Proxies added while mode=None are silently ignored by build_proxy_pool().
							if (app.pipeline.proxy_settings.proxy_mode === 'None') {
								app.pipeline.proxy_settings.proxy_mode = 'Rotate';
								console.log('[DataPanel] proxy_mode was None — auto-switched to Rotate');
							}
							console.log('[DataPanel] added proxy source:', app.proxyPath, '| mode:', app.pipeline.proxy_settings.proxy_mode, '| total sources:', app.pipeline.proxy_settings.proxy_sources.length);
						}}
					>+ Add</button>
				</div>

				{#if app.pipeline.proxy_settings.proxy_mode === 'CpmLimited'}
					<div class="flex items-center justify-between bg-background rounded p-2 border border-border">
						<div>
							<span class="text-[10px] text-muted-foreground">CPM per proxy</span>
							<p class="text-[9px] text-muted-foreground/60">Max checks per minute per proxy</p>
						</div>
						<input type="number" min="1"
							bind:value={app.pipeline.proxy_settings.cpm_per_proxy}
							class="w-16 skeu-input text-[10px] text-center"
							placeholder="60"
						/>
					</div>
				{/if}

				<div class="bg-background rounded p-2 border border-border space-y-1.5 mt-1">
					<div class="flex items-center justify-between">
						<div>
							<span class="text-[10px] text-muted-foreground">Ban duration</span>
							<p class="text-[9px] text-muted-foreground/60">Seconds to ban a proxy after failures</p>
						</div>
						<div class="flex items-center gap-1">
							<input type="number" value={app.pipeline.proxy_settings.ban_duration_secs}
								class="w-16 skeu-input text-[10px] text-center"
								oninput={(e) => updateBanDuration((e.target as HTMLInputElement).value)} />
							<span class="text-[9px] text-muted-foreground">s</span>
						</div>
					</div>
					<div class="flex items-center justify-between">
						<div>
							<span class="text-[10px] text-muted-foreground">Max retries before ban</span>
							<p class="text-[9px] text-muted-foreground/60">Failures before proxy is banned</p>
						</div>
						<input type="number" value={app.pipeline.proxy_settings.max_retries_before_ban}
							class="w-16 skeu-input text-[10px] text-center"
							oninput={(e) => updateMaxRetries((e.target as HTMLInputElement).value)} />
					</div>
					<div class="flex items-center justify-between">
						<div>
							<span class="text-[10px] text-muted-foreground">Concurrent per proxy</span>
							<p class="text-[9px] text-muted-foreground/60">0 = unlimited</p>
						</div>
						<input type="number" value={app.pipeline.runner_settings.concurrent_per_proxy}
							class="w-16 skeu-input text-[10px] text-center"
							oninput={(e) => updateConcurrentPerProxy((e.target as HTMLInputElement).value)} />
					</div>
				</div>
			{/if}
		</div>
	</div>

	<!-- Proxy Groups -->
	{#if app.pipeline.proxy_settings.proxy_mode !== 'None'}
		<div>
			<div class="text-[10px] uppercase tracking-wider text-muted-foreground mb-1.5 flex items-center gap-1.5">
				<Shield size={11} />
				Proxy Groups
			</div>
			<div class="space-y-1.5">
				<!-- Active group selector -->
				{#if app.pipeline.proxy_settings.proxy_groups.length > 0}
					<SkeuSelect
						value={app.pipeline.proxy_settings.active_group || ''}
						onValueChange={(v) => { app.pipeline.proxy_settings.active_group = v; }}
						options={[
							{ value: '', label: 'Default (above sources)' },
							...app.pipeline.proxy_settings.proxy_groups.map(g => ({ value: g.name, label: g.name })),
						]}
						class="w-full text-[11px]"
					/>
				{/if}

				<!-- Group cards -->
				{#each app.pipeline.proxy_settings.proxy_groups as group, gi}
					<div class="bg-background rounded p-1.5 border border-border space-y-1">
						<div class="flex items-center gap-1">
							<span class="text-[10px] font-medium text-foreground flex-1 truncate">{group.name}</span>
							<span class="text-[9px] text-muted-foreground/60">{group.sources.length} src</span>
							<button class="p-0.5 text-muted-foreground hover:text-red" onclick={() => removeProxyGroup(gi)} title="Remove group">
								<Trash2 size={10} />
							</button>
						</div>
						<SkeuSelect
							value={group.mode}
							onValueChange={(v) => {
								const groups = [...app.pipeline.proxy_settings.proxy_groups];
								groups[gi] = { ...groups[gi], mode: v as any };
								app.pipeline.proxy_settings.proxy_groups = groups;
							}}
							options={[{value:'Rotate',label:'Rotate'},{value:'Sticky',label:'Sticky'},{value:'CpmLimited',label:'CPM Limited'}]}
							class="w-full text-[9px]"
						/>
						{#each group.sources as src, si}
							<div class="flex items-center gap-1">
								<span class="text-[9px] text-muted-foreground/60 w-5 shrink-0">{src.source_type === 'Url' ? 'URL' : src.source_type === 'File' ? 'File' : 'Inl'}</span>
								<input type="text" bind:value={src.value} placeholder="proxy source..." class="flex-1 skeu-input text-[9px] font-mono" />
								<button class="p-0.5 text-muted-foreground hover:text-red" onclick={() => removeGroupSource(gi, si)}><Trash2 size={9} /></button>
							</div>
						{/each}
						<div class="flex gap-0.5">
							<button class="skeu-btn text-[8px] flex-1" onclick={() => addGroupSource(gi, 'File')}>+ File</button>
							<button class="skeu-btn text-[8px] flex-1" onclick={() => addGroupSource(gi, 'Url')}>+ URL</button>
							<button class="skeu-btn text-[8px] flex-1" onclick={() => addGroupSource(gi, 'Inline')}>+ Inline</button>
						</div>
					</div>
				{/each}

				<!-- Add new group -->
				<div class="flex gap-1">
					<input type="text" bind:value={newGroupName} placeholder="New group name..."
						class="flex-1 skeu-input text-[10px]"
						onkeydown={(e) => { if (e.key === 'Enter') addProxyGroup(); }} />
					<button class="skeu-btn text-[9px] flex items-center gap-0.5" onclick={addProxyGroup}>
						<Plus size={9} /> Add
					</button>
				</div>
			</div>
		</div>
	{/if}

	<!-- Browser Settings -->
	<div>
		<div class="text-[10px] uppercase tracking-wider text-muted-foreground mb-1.5 flex items-center gap-1.5">
			<Globe size={11} />
			Browser Settings
		</div>
		<div class="space-y-1.5">
			<div>
				<label class="text-[10px] text-muted-foreground">Browser Profile</label>
				<SkeuSelect value={app.pipeline.browser_settings.browser}
					onValueChange={(v) => { app.pipeline.browser_settings.browser = v; }}
					options={[{value:'chrome',label:'Chrome'},{value:'firefox',label:'Firefox'},{value:'safari',label:'Safari'},{value:'edge',label:'Edge'},{value:'custom',label:'Custom'}]}
					class="w-full text-[11px] mt-0.5"
				/>
			</div>
			<div>
				<label class="text-[10px] text-muted-foreground">User Agent</label>
				<input type="text"
					value={app.pipeline.browser_settings.user_agent || ''}
					placeholder="Auto"
					class="w-full skeu-input text-[11px] font-mono mt-0.5"
					oninput={(e) => { app.pipeline.browser_settings.user_agent = (e.target as HTMLInputElement).value || null; }}
				/>
			</div>
			<div>
				<label class="text-[10px] text-muted-foreground">JA3 Fingerprint</label>
				<input type="text"
					value={app.pipeline.browser_settings.ja3 || ''}
					placeholder="Auto (derived from browser profile)"
					class="w-full skeu-input text-[11px] font-mono mt-0.5"
					oninput={(e) => { app.pipeline.browser_settings.ja3 = (e.target as HTMLInputElement).value || null; }}
				/>
				<p class="text-[9px] text-muted-foreground mt-0.5">TLS client hello fingerprint string</p>
			</div>
			<div>
				<label class="text-[10px] text-muted-foreground">HTTP/2 Fingerprint</label>
				<input type="text"
					value={app.pipeline.browser_settings.http2_fingerprint || ''}
					placeholder="Auto (derived from browser profile)"
					class="w-full skeu-input text-[11px] font-mono mt-0.5"
					oninput={(e) => { app.pipeline.browser_settings.http2_fingerprint = (e.target as HTMLInputElement).value || null; }}
				/>
				<p class="text-[9px] text-muted-foreground mt-0.5">HTTP/2 SETTINGS frame + priority fingerprint</p>
			</div>
		</div>
	</div>

	<!-- ── Hits Database ──────────────────────────────────────────────── -->
	<div>
		<div class="text-[10px] uppercase tracking-wider text-muted-foreground mb-1.5 flex items-center gap-1.5">
			<Database size={11} />
			Hits Database
		</div>
		<div class="space-y-1.5">
			<!-- Job selector -->
			<div>
				<label class="text-[10px] text-muted-foreground">Job</label>
				<SkeuSelect
					value={selectedHitsJobId ?? ''}
					onValueChange={(v) => selectHitsJob(v)}
					options={[
						{ value: '', label: 'Select a job…' },
						...app.jobs.map((j: any) => ({ value: j.id, label: `${j.name} [${j.state}]` }))
					]}
					class="w-full text-[11px] mt-0.5"
				/>
			</div>

			{#if selectedHitsJobId}
				<!-- Stats + actions row -->
				<div class="flex items-center justify-between">
					<span class="text-[9px] text-muted-foreground">
						{hitsForJob.length} hit{hitsForJob.length !== 1 ? 's' : ''}
					</span>
					<div class="flex gap-1">
						<button
							class="skeu-btn text-[9px] px-1.5 py-0.5 flex items-center gap-0.5"
							onclick={refreshHits}
							title="Refresh from backend"
						>
							<RefreshCw size={9} />
							Refresh
						</button>
						<button
							class="skeu-btn text-[9px] px-1.5 py-0.5 flex items-center gap-0.5"
							onclick={exportHitsTxt}
							title="Export hits as TXT"
							disabled={hitsForJob.length === 0}
						>
							<Download size={9} />
							TXT
						</button>
						<button
							class="skeu-btn text-[9px] px-1.5 py-0.5 flex items-center gap-0.5"
							onclick={exportHitsCsv}
							title="Export hits as CSV"
							disabled={hitsForJob.length === 0}
						>
							<Download size={9} />
							CSV
						</button>
					</div>
				</div>

				{#if hitsForJob.length > 0}
					<!-- Hits table (last 100 in reverse order — newest first) -->
					<div class="bg-background border border-border rounded overflow-hidden">
						<div class="overflow-auto max-h-52">
							<table class="w-full text-[10px]">
								<thead class="sticky top-0 bg-surface border-b border-border z-10">
									<tr>
										<th class="px-2 py-1 text-left font-medium text-muted-foreground">Data</th>
										<th class="px-2 py-1 text-left font-medium text-muted-foreground">Captures</th>
									</tr>
								</thead>
								<tbody>
									{#each hitsForJob.slice(-100) as hit, i (`${selectedHitsJobId}-${hitsForJob.length - 100 + i}`)}
										<tr class="border-b border-border/40 hover:bg-surface/60">
											<td class="px-2 py-0.5 font-mono text-[9px] text-green truncate max-w-[130px]" title={hit.data_line}>
												{hit.data_line}
											</td>
											<td class="px-2 py-0.5 font-mono text-[9px] text-muted-foreground truncate max-w-[160px]" title={Object.entries(hit.captures ?? {}).map(([k,v]) => `${k}=${v}`).join(' | ')}>
												{Object.entries(hit.captures ?? {}).map(([k,v]) => `${k}=${v}`).join(' | ')}
											</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
						{#if hitsForJob.length > 100}
							<div class="px-2 py-1 text-[9px] text-muted-foreground bg-surface border-t border-border">
								Showing last 100 of {hitsForJob.length} — export for full list
							</div>
						{/if}
					</div>
				{:else}
					<p class="text-[10px] text-muted-foreground text-center py-3">No hits recorded for this job yet</p>
				{/if}
			{:else}
				<p class="text-[10px] text-muted-foreground text-center py-2">Select a job to view its hits</p>
			{/if}
		</div>
	</div>

</div>