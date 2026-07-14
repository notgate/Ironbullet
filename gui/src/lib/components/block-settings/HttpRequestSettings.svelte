<script lang="ts">
	import type { Block } from '$lib/types';
	import SkeuSelect from '../SkeuSelect.svelte';
	import VariableInput from '../VariableInput.svelte';
	import Plus from '@lucide/svelte/icons/plus';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { inputCls, labelCls, hintCls, hasVars, HTTP_VERSION_OPTIONS } from './shared';
	import { intellisense } from '$lib/intellisense-action.svelte';
	import { app } from '$lib/state.svelte';

	let { block, updateSettings, embedBadge }: {
		block: Block;
		updateSettings: (key: string, value: unknown) => void;
		embedBadge: import('svelte').Snippet<[string | undefined]>;
	} = $props();

	// --- HTTP Request: header helpers ---
	function headersToRaw(headers: [string, string][]): string {
		return headers.map(([k, v]) => `${k}: ${v}`).join('\n');
	}

	function rawToHeaders(raw: string): [string, string][] {
		return raw.split('\n')
			.map(line => line.trim())
			.filter(line => line.length > 0)
			.map(line => {
				const idx = line.indexOf(':');
				if (idx === -1) return [line, ''] as [string, string];
				return [line.slice(0, idx).trim(), line.slice(idx + 1).trim()] as [string, string];
			});
	}

	let httpTab = $state<'headers' | 'body' | 'cookies' | 'options'>('headers');
	let rawHeaders = $state('');
	let headersViewMode = $state<'raw' | 'kv'>('raw');

	$effect(() => {
		if (block && block.settings.type === 'HttpRequest') {
			rawHeaders = headersToRaw(block.settings.headers);
		}
	});

	function commitRawHeaders() {
		if (!block || block.settings.type !== 'HttpRequest') return;
		updateSettings('headers', rawToHeaders(rawHeaders));
	}

	function addHeader() {
		if (!block || block.settings.type !== 'HttpRequest') return;
		const headers: [string, string][] = [...block.settings.headers, ['', '']];
		updateSettings('headers', headers);
	}

	function removeHeader(idx: number) {
		if (!block || block.settings.type !== 'HttpRequest') return;
		const headers = block.settings.headers.filter((_: [string, string], i: number) => i !== idx);
		updateSettings('headers', headers);
	}

	function updateHeaderKey(idx: number, key: string) {
		if (!block || block.settings.type !== 'HttpRequest') return;
		const headers: [string, string][] = [...block.settings.headers];
		headers[idx] = [key, headers[idx][1]];
		updateSettings('headers', headers);
	}

	function updateHeaderValue(idx: number, value: string) {
		if (!block || block.settings.type !== 'HttpRequest') return;
		const headers: [string, string][] = [...block.settings.headers];
		headers[idx] = [headers[idx][0], value];
		updateSettings('headers', headers);
	}

	// --- Copy as curl/PowerShell ---
	let copyNotice = $state('');

	function generateCurl(): string {
		if (!block || block.settings.type !== 'HttpRequest') return '';
		const s = block.settings;
		const parts = ['curl'];
		if (s.method !== 'GET') parts.push(`-X ${s.method}`);
		for (const [k, v] of s.headers) {
			parts.push(`-H '${k}: ${v}'`);
		}
		if (s.body && s.body_type !== 'None') {
			parts.push(`-d '${s.body.replace(/'/g, "'\\''")}'`);
		}
		if (s.follow_redirects && s.auto_redirect) parts.push('-L');
		parts.push(`'${s.url}'`);
		return parts.join(' \\\n  ');
	}

	function generatePowershell(): string {
		if (!block || block.settings.type !== 'HttpRequest') return '';
		const s = block.settings;
		const parts = ['Invoke-WebRequest'];
		parts.push(`-Method ${s.method}`);
		parts.push(`-Uri "${s.url}"`);
		if (s.headers.length > 0) {
			const hdrs = s.headers.map(([k, v]: [string, string]) => `"${k}"="${v}"`).join('; ');
			parts.push(`-Headers @{${hdrs}}`);
		}
		if (s.body && s.body_type !== 'None') {
			parts.push(`-Body "${s.body.replace(/"/g, '`"')}"`);
		}
		if (s.content_type) {
			parts.push(`-ContentType "${s.content_type}"`);
		}
		return parts.join(' `\n  ');
	}

	function copyAsCurl() {
		navigator.clipboard.writeText(generateCurl());
		copyNotice = 'Copied as curl!';
		setTimeout(() => { copyNotice = ''; }, 1500);
	}

	function copyAsPowershell() {
		navigator.clipboard.writeText(generatePowershell());
		copyNotice = 'Copied as PowerShell!';
		setTimeout(() => { copyNotice = ''; }, 1500);
	}
</script>

{#if block.settings.type === 'HttpRequest'}
	<!-- URL bar -->
	<div class="flex gap-1.5">
		<SkeuSelect
			value={block.settings.method}
			onValueChange={(v) => updateSettings('method', v)}
			options={[{value:'GET',label:'GET'},{value:'POST',label:'POST'},{value:'PUT',label:'PUT'},{value:'DELETE',label:'DELETE'},{value:'PATCH',label:'PATCH'},{value:'HEAD',label:'HEAD'},{value:'OPTIONS',label:'OPTIONS'}]}
			class="text-[11px] w-20"
		/>
		<VariableInput
			value={block.settings.url}
			placeholder="https://example.com/api/endpoint"
			context="url"
			class="flex-1 skeu-input text-[11px] font-mono"
			oninput={(e) => updateSettings('url', (e.target as HTMLInputElement).value)}
		/>
	</div>

	<!-- Tabbed sub-panel -->
	<div class="flex border-b border-border">
		{#each ['headers', 'body', 'cookies', 'options'] as tab}
			<button
				class="px-2 py-0.5 text-[11px] capitalize {httpTab === tab ? 'text-foreground font-medium' : 'text-muted-foreground hover:text-foreground'} transition-colors"
				onclick={() => { httpTab = tab as typeof httpTab; }}
			>{tab}</button>
		{/each}
	</div>

	{#if httpTab === 'headers'}
		<!-- View mode toggle -->
		<div class="flex items-center gap-1 mb-1">
			<button
				class="text-[10px] px-1.5 py-0.5 rounded {headersViewMode === 'raw' ? 'bg-secondary text-foreground' : 'text-muted-foreground hover:text-foreground'}"
				onclick={() => { headersViewMode = 'raw'; }}
			>Raw</button>
			<button
				class="text-[10px] px-1.5 py-0.5 rounded {headersViewMode === 'kv' ? 'bg-secondary text-foreground' : 'text-muted-foreground hover:text-foreground'}"
				onclick={() => { headersViewMode = 'kv'; if (block && block.settings.type === 'HttpRequest') commitRawHeaders(); }}
			>Key-Value</button>
		</div>

		{#if headersViewMode === 'raw'}
			<div class="relative">
				<textarea
					class="w-full skeu-input text-[11px] font-mono min-h-[120px] resize-y"
					placeholder="Content-Type: application/json&#10;Accept: */*&#10;Authorization: Bearer <token>"
					bind:value={rawHeaders}
					onblur={commitRawHeaders}
					use:intellisense={{ context: 'header_value', responseBody: app.lastDebugResponseHeaders }}
				></textarea>
				{@render embedBadge(rawHeaders)}
			</div>
			<p class="text-[10px] text-muted-foreground">One header per line: <code class="text-foreground/70">Name: Value</code></p>
		{:else}
			<!-- Key-Value pairs -->
			<div class="space-y-1">
				{#each block.settings.headers as header, hi}
					<div class="flex gap-1 items-center">
						<VariableInput
							value={header[0]}
							placeholder="Header name"
							context="header_name"
							class="flex-1 skeu-input text-[10px] font-mono"
							oninput={(e) => updateHeaderKey(hi, (e.target as HTMLInputElement).value)}
						/>
						<VariableInput
							value={header[1]}
							placeholder="Value"
							context="header_value"
							responseBody={header[0]}
							class="flex-1 skeu-input text-[10px] font-mono"
							oninput={(e) => updateHeaderValue(hi, (e.target as HTMLInputElement).value)}
						/>
						<button class="p-0.5 text-muted-foreground hover:text-red shrink-0" onclick={() => removeHeader(hi)} title="Remove">
							<Trash2 size={10} />
						</button>
					</div>
				{/each}
			</div>
			<button class="flex items-center gap-1 text-[10px] text-primary hover:underline mt-1" onclick={addHeader}>
				<Plus size={10} /> Add Header
			</button>
		{/if}
	{:else if httpTab === 'body'}
		<div class="flex items-center gap-2 mb-1">
			<span class={labelCls}>Mode</span>
			<SkeuSelect value={block.settings.body_type}
				onValueChange={(v) => updateSettings('body_type', v)}
				options={[{value:'None',label:'None'},{value:'Standard',label:'Form'},{value:'Raw',label:'Raw'},{value:'Multipart',label:'Multipart'}]}
				class="text-[10px]"
			/>
		</div>
		{#if block.settings.body_type !== 'None'}
			<div class="relative">
				<textarea value={block.settings.body} placeholder="Request body..."
					class="w-full skeu-input text-[11px] font-mono min-h-[100px] resize-y"
					oninput={(e) => updateSettings('body', (e.target as HTMLTextAreaElement).value)}
					use:intellisense={{ context: 'generic', responseBody: app.lastDebugResponseBody }}
				></textarea>
				{@render embedBadge(block.settings.body)}
			</div>
		{/if}
	{:else if httpTab === 'cookies'}
		<p class={hintCls}>Custom cookies to send with the request. One per line: <code class="text-foreground/70">name=value</code></p>
		<div class="relative">
			<textarea
				value={block.settings.custom_cookies || ''}
				placeholder="session_id=abc123&#10;csrf_token=xyz789&#10;auth=<COOKIES>"
				class="w-full skeu-input text-[11px] font-mono min-h-[100px] resize-y mt-1"
				oninput={(e) => updateSettings('custom_cookies', (e.target as HTMLTextAreaElement).value)}
				use:intellisense={{ context: 'generic', responseBody: app.lastDebugResponseHeaders }}
			></textarea>
			{@render embedBadge(block.settings.custom_cookies)}
		</div>
		<p class="text-[9px] text-muted-foreground mt-1">Cookies are joined with <code class="text-foreground/70">; </code> and sent as the <code class="text-foreground/70">Cookie</code> header. Supports <code class="text-foreground/70">&lt;VARIABLE&gt;</code> interpolation.</p>
	{:else if httpTab === 'options'}
		<div class="space-y-1.5">
			<label class="flex items-center gap-2 text-[11px] text-foreground">
				<input type="checkbox" checked={block.settings.follow_redirects} onchange={() => updateSettings('follow_redirects', !block!.settings.follow_redirects)} class="skeu-checkbox" />
				Follow redirects (auto-follow 3xx)
			</label>
			<div class="flex items-center gap-2">
				<span class="text-[11px] text-muted-foreground">Timeout:</span>
				<input type="number" value={block.settings.timeout_ms}
					class="w-20 skeu-input text-[11px]"
					oninput={(e) => updateSettings('timeout_ms', parseInt((e.target as HTMLInputElement).value))} />
				<span class="text-[10px] text-muted-foreground">ms</span>
			</div>
			<div class="flex items-center gap-2">
				<span class="text-[11px] text-muted-foreground">HTTP Version:</span>
				<SkeuSelect
					value={block.settings.http_version || 'HTTP/1.1'}
					onValueChange={(v) => updateSettings('http_version', v)}
					options={HTTP_VERSION_OPTIONS}
					class="text-[10px]"
				/>
			</div>
			<div class="my-1.5 groove-h h-px"></div>
			<div>
				<label class={labelCls}>Response variable</label>
				<VariableInput
					value={block.settings.response_var || 'SOURCE'}
					placeholder="SOURCE"
					class={inputCls}
					oninput={(e) => updateSettings('response_var', (e.target as HTMLInputElement).value)}
				/>
				<p class="text-[9px] text-muted-foreground mt-0.5">Body → <code class="text-foreground/70">data.{'{'}var{'}'}</code> &nbsp; Headers → <code class="text-foreground/70">data.{'{'}var{'}'}.HEADERS</code> &nbsp; Cookies → <code class="text-foreground/70">data.{'{'}var{'}'}.COOKIES</code></p>
			</div>
		</div>
	{/if}

	<!-- TLS Client + SSL -->
	<div class="my-1.5 groove-h h-px"></div>
	<div class="space-y-1">

		<!-- TLS client selector -->
		<div>
			<label class={labelCls}>TLS Client</label>
			<SkeuSelect
				value={block.settings.tls_client || 'RustTLS'}
				onValueChange={(v) => updateSettings('tls_client', v)}
				options={[
					{ value: 'RustTLS',  label: 'RustTLS — reqwest + rustls (standard HTTPS)' },
					{ value: 'WreqTLS',  label: 'WreqTLS — wreq + BoringSSL (browser emulation)' },
					{ value: 'AzureTLS', label: 'AzureTLS — Go sidecar (JA3 + custom fingerprints)' },
				]}
				placeholder="Select TLS client..."
			/>
			{#if (block.settings.tls_client || 'RustTLS') === 'RustTLS'}
				<p class="text-[9px] text-muted-foreground mt-0.5">reqwest + rustls — standard HTTPS, no fingerprinting. Fastest for APIs and internal targets.</p>
			{:else if (block.settings.tls_client || 'RustTLS') === 'WreqTLS'}
				<p class="text-[9px] text-muted-foreground mt-0.5">wreq + BoringSSL — 100+ browser device emulation profiles with accurate JA3/JA4/HTTP2 fingerprints. No sidecar required.</p>
			{:else}
				<p class="text-[9px] text-muted-foreground mt-0.5">Go sidecar (azuretls) — custom JA3/JA4 strings, browser TLS imitation, per-block HTTP/2 fingerprinting, custom cipher suites.</p>
			{/if}
		</div>

		<label class="flex items-center gap-2 text-[11px] text-foreground">
			<input type="checkbox" checked={block.settings.ssl_verify !== false}
				onchange={() => updateSettings('ssl_verify', block.settings.ssl_verify === false ? true : false)}
				class="skeu-checkbox" />
			Verify TLS certificate
			{#if block.settings.ssl_verify === false}
				<span class="text-[9px] text-orange bg-orange/10 px-1 rounded">⚠ insecure</span>
			{/if}
		</label>
		<p class="text-[9px] text-muted-foreground">Uncheck for self-signed certs or TLS debugging (SEC_E_ILLEGAL_MESSAGE / handshake errors)</p>

		<label class="flex items-center gap-2 text-[11px] text-foreground mt-1">
			<input type="checkbox" checked={block.settings.proxy_insecure === true}
				onchange={() => updateSettings('proxy_insecure', !block.settings.proxy_insecure)}
				class="skeu-checkbox" />
			Accept invalid proxy TLS certificate
			{#if block.settings.proxy_insecure}
				{#if (block.settings.tls_client || 'RustTLS') !== 'AzureTLS'}
					<span class="text-[9px] text-orange bg-orange/10 px-1 rounded">⚠ also affects target TLS</span>
				{:else}
					<span class="text-[9px] text-yellow-500/80 bg-yellow-500/10 px-1 rounded">no-op on AzureTLS</span>
				{/if}
			{/if}
		</label>
		<p class="text-[9px] text-muted-foreground">For HTTPS proxies with self-signed certs (<code class="text-foreground/70">curl --proxy-insecure</code>). On RustTLS/WreqTLS this also skips target TLS verification — use AzureTLS to avoid this.</p>

		<!-- WreqTLS emulation profile -->
		{#if (block.settings.tls_client || 'RustTLS') === 'WreqTLS'}
		<div>
			<label class={labelCls}>Browser Emulation Profile <span class="text-muted-foreground/60">(WreqTLS · TLS + HTTP/2 fingerprint)</span></label>
			<SkeuSelect
				value={block.settings.wreq_emulation || 'Chrome134'}
				onValueChange={(v) => updateSettings('wreq_emulation', v)}
				options={[
					{ value: 'Chrome137', label: 'Chrome 137' },
					{ value: 'Chrome136', label: 'Chrome 136' },
					{ value: 'Chrome135', label: 'Chrome 135' },
					{ value: 'Chrome134', label: 'Chrome 134 (default)' },
					{ value: 'Chrome133', label: 'Chrome 133' },
					{ value: 'Chrome132', label: 'Chrome 132' },
					{ value: 'Chrome131', label: 'Chrome 131' },
					{ value: 'Chrome130', label: 'Chrome 130' },
					{ value: 'Chrome128', label: 'Chrome 128' },
					{ value: 'Chrome127', label: 'Chrome 127' },
					{ value: 'Chrome124', label: 'Chrome 124' },
					{ value: 'Chrome120', label: 'Chrome 120' },
					{ value: 'Chrome116', label: 'Chrome 116' },
					{ value: 'Chrome110', label: 'Chrome 110' },
					{ value: 'Chrome107', label: 'Chrome 107' },
					{ value: 'Edge134', label: 'Edge 134' },
					{ value: 'Edge131', label: 'Edge 131' },
					{ value: 'Edge127', label: 'Edge 127' },
					{ value: 'Edge122', label: 'Edge 122' },
					{ value: 'Edge101', label: 'Edge 101' },
					{ value: 'Firefox139', label: 'Firefox 139' },
					{ value: 'Firefox136', label: 'Firefox 136' },
					{ value: 'Firefox135', label: 'Firefox 135' },
					{ value: 'Firefox133', label: 'Firefox 133' },
					{ value: 'Firefox128', label: 'Firefox 128' },
					{ value: 'Firefox117', label: 'Firefox 117' },
					{ value: 'Firefox109', label: 'Firefox 109' },
					{ value: 'Safari18_3_1', label: 'Safari 18.3.1' },
					{ value: 'Safari18_3', label: 'Safari 18.3' },
					{ value: 'Safari18_2', label: 'Safari 18.2' },
					{ value: 'Safari18', label: 'Safari 18' },
					{ value: 'Safari17_5', label: 'Safari 17.5' },
					{ value: 'Safari17_4_1', label: 'Safari 17.4.1' },
					{ value: 'Safari17_0', label: 'Safari 17.0' },
					{ value: 'Safari16_5', label: 'Safari 16.5' },
					{ value: 'Safari16', label: 'Safari 16' },
					{ value: 'Safari15_6_1', label: 'Safari 15.6.1' },
					{ value: 'OkHttp5', label: 'OkHttp 5 (Android)' },
					{ value: 'OkHttp4_12', label: 'OkHttp 4.12 (Android)' },
					{ value: 'OkHttp4_10', label: 'OkHttp 4.10 (Android)' },
					{ value: 'OkHttp3_14', label: 'OkHttp 3.14 (Android)' },
					{ value: 'Opera119', label: 'Opera 119' },
					{ value: 'Opera118', label: 'Opera 118' },
					{ value: 'Opera117', label: 'Opera 117' },
					{ value: 'Opera116', label: 'Opera 116' },
				]}
			/>
			<p class="text-[9px] text-muted-foreground mt-0.5">Sets the full TLS ClientHello, cipher suite order, HTTP/2 SETTINGS frame, and HPACK headers to match the selected browser. Verify at <span class="text-primary">tls.peet.ws</span>.</p>
		</div>
		{/if}

		<!-- AzureTLS fingerprinting settings -->
		{#if (block.settings.tls_client || 'RustTLS') === 'AzureTLS'}
		<!-- Browser TLS profile -->
		<div>
			<label class={labelCls}>Browser Profile <span class="text-muted-foreground/60">(AzureTLS · TLS + HTTP/2 fingerprint)</span></label>
			<SkeuSelect
				value={block.settings.browser_profile || ''}
				options={[
					{ value: '',        label: 'Inherit from pipeline settings' },
					{ value: 'chrome',  label: 'Chrome — most common, bypasses most WAFs' },
					{ value: 'firefox', label: 'Firefox — good alternative fingerprint' },
					{ value: 'safari',  label: 'Safari — iOS/macOS fingerprint' },
					{ value: 'edge',    label: 'Edge — Chromium-based Microsoft Edge' },
				]}
				onValueChange={(v) => updateSettings('browser_profile', v)}
			/>
			<p class="text-[9px] text-muted-foreground mt-0.5">Sets the TLS cipher order, extensions, and HTTP/2 SETTINGS frame to match the selected browser. Overrides pipeline-level Browser setting for this block only.</p>
		</div>
		<!-- Per-block JA3 override -->
		<div>
			<label class={labelCls}>JA3 Override <span class="text-muted-foreground/60">(AzureTLS · optional)</span></label>
			<input
				type="text"
				value={block.settings.ja3_override || ''}
				placeholder="Leave empty to use pipeline JA3 or browser profile default"
				class="w-full skeu-input text-[10px] font-mono mt-0.5"
				oninput={(e) => updateSettings('ja3_override', (e.target as HTMLInputElement).value.trim())}
			/>
			<p class="text-[9px] text-muted-foreground mt-0.5">Format: TLSVersion,Ciphers,Extensions,EllipticCurves,PointFormats — e.g. verify at <span class="text-primary">tls.peet.ws</span></p>
		</div>
		<!-- Per-block HTTP/2 fingerprint override -->
		<div>
			<label class={labelCls}>HTTP/2 Fingerprint Override <span class="text-muted-foreground/60">(AzureTLS · optional)</span></label>
			<input
				type="text"
				value={block.settings.http2fp_override || ''}
				placeholder="Leave empty to use pipeline HTTP/2 fingerprint"
				class="w-full skeu-input text-[10px] font-mono mt-0.5"
				oninput={(e) => updateSettings('http2fp_override', (e.target as HTMLInputElement).value.trim())}
			/>
		</div>
		<!-- Custom Cipher Suites -->
		<div>
			<label class={labelCls}>Custom Cipher Suites <span class="text-muted-foreground/60">(AzureTLS · optional)</span></label>
			<textarea
				value={block.settings.cipher_suites || ''}
				rows={2}
				placeholder={'Leave empty to use browser profile defaults.\nDash-separated IANA IDs, e.g:\n4865-4866-4867-49195-49199-49196-49200-52393-52392'}
				class="w-full skeu-input text-[10px] font-mono resize-y mt-0.5"
				oninput={(e) => updateSettings('cipher_suites', (e.target as HTMLTextAreaElement).value.trim())}
			></textarea>
			<details class="mt-0.5">
				<summary class="text-[9px] text-primary/70 cursor-pointer select-none">Common cipher IDs reference</summary>
				<div class="text-[9px] text-muted-foreground font-mono space-y-0.5 mt-1 bg-background/50 rounded p-1.5 leading-relaxed">
					<div><span class="text-foreground/80">4865</span> TLS_AES_128_GCM_SHA256 (TLS 1.3)</div>
					<div><span class="text-foreground/80">4866</span> TLS_AES_256_GCM_SHA384 (TLS 1.3)</div>
					<div><span class="text-foreground/80">4867</span> TLS_CHACHA20_POLY1305_SHA256 (TLS 1.3)</div>
					<div><span class="text-foreground/80">49195</span> TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256</div>
					<div><span class="text-foreground/80">49199</span> TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256</div>
					<div><span class="text-foreground/80">49196</span> TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384</div>
					<div><span class="text-foreground/80">49200</span> TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384</div>
					<div><span class="text-foreground/80">52393</span> TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305</div>
					<div><span class="text-foreground/80">52392</span> TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305</div>
					<div><span class="text-foreground/80">49171</span> TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA</div>
					<div><span class="text-foreground/80">49172</span> TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA</div>
					<div><span class="text-foreground/80">156</span> TLS_RSA_WITH_AES_128_GCM_SHA256</div>
					<div><span class="text-foreground/80">157</span> TLS_RSA_WITH_AES_256_GCM_SHA384</div>
					<div><span class="text-foreground/80">47</span> TLS_RSA_WITH_AES_128_CBC_SHA</div>
					<div><span class="text-foreground/80">53</span> TLS_RSA_WITH_AES_256_CBC_SHA</div>
				</div>
			</details>
		</div>
		{/if}
	</div>

	<!-- Copy as curl/PowerShell -->
	<div class="my-1.5 groove-h h-px"></div>
	<div class="flex gap-1.5">
		<button class="skeu-btn text-[10px] flex-1" onclick={copyAsCurl}>Copy as curl</button>
		<button class="skeu-btn text-[10px] flex-1" onclick={copyAsPowershell}>Copy as PowerShell</button>
	</div>
	{#if copyNotice}
		<div class="text-[9px] text-green text-center">{copyNotice}</div>
	{/if}
{/if}
