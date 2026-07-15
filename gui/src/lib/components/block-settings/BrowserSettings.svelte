<script lang="ts">
	import type { Block } from '$lib/types';
	import SkeuSelect from '../SkeuSelect.svelte';
	import VariableInput from '../VariableInput.svelte';
	import { intellisense } from '$lib/intellisense-action.svelte';
	import { app } from '$lib/state.svelte';
	import { inputCls, labelCls, hasVars } from './shared';

	let { block, updateSettings, embedBadge }: {
		block: Block;
		updateSettings: (key: string, value: unknown) => void;
		embedBadge: import('svelte').Snippet<[string | undefined]>;
	} = $props();
</script>

<!-- ===================== BROWSER OPEN ===================== -->
{#if block.settings.type === 'BrowserOpen'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Browser</label>
			<SkeuSelect value={block.settings.browser_type}
				onValueChange={(v) => updateSettings('browser_type', v)}
				options={[{value:'chromium',label:'Chromium'},{value:'firefox',label:'Firefox'},{value:'webkit',label:'WebKit'}]}
				class="w-full mt-0.5"
			/>
		</div>
		<label class="flex items-center gap-2 text-[11px] text-foreground">
			<input type="checkbox" checked={block.settings.headless} onchange={() => updateSettings('headless', !block!.settings.headless)} class="skeu-checkbox" />
			Headless mode
		</label>
		<div>
			<label class={labelCls}>Proxy (optional)</label>
			<VariableInput value={block.settings.proxy} placeholder="http://user:pass@host:port" class={inputCls}
				oninput={(e) => updateSettings('proxy', (e.target as HTMLInputElement).value)} />
		</div>
		<div>
			<label class={labelCls}>Extra args</label>
			<VariableInput value={block.settings.extra_args} placeholder="--disable-gpu --no-sandbox" class={inputCls}
				oninput={(e) => updateSettings('extra_args', (e.target as HTMLInputElement).value)} />
			<p class="text-[9px] text-muted-foreground mt-0.5">Space-separated browser launch flags</p>
		</div>
	</div>

<!-- ===================== NAVIGATE TO ===================== -->
{:else if block.settings.type === 'NavigateTo'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>URL</label>
			<VariableInput value={block.settings.url} placeholder="https://example.com" class={inputCls}
				oninput={(e) => updateSettings('url', (e.target as HTMLInputElement).value)} />
		</div>
		<div>
			<label class={labelCls}>Wait until</label>
			<SkeuSelect value={block.settings.wait_until}
				onValueChange={(v) => updateSettings('wait_until', v)}
				options={[{value:'load',label:'Page Load'},{value:'domcontentloaded',label:'DOM Ready'},{value:'networkidle',label:'Network Idle'}]}
				class="w-full mt-0.5"
			/>
		</div>
		<div class="flex items-center gap-2">
			<span class="text-[11px] text-muted-foreground">Timeout:</span>
			<input type="number" value={block.settings.timeout_ms}
				class="w-20 skeu-input text-[11px]"
				oninput={(e) => updateSettings('timeout_ms', parseInt((e.target as HTMLInputElement).value))} />
			<span class="text-[10px] text-muted-foreground">ms</span>
		</div>
		<div>
			<label class={labelCls}>Custom cookies</label>
			<textarea value={block.settings.custom_cookies || ''} placeholder={"session=abc123\nauth=<TOKEN>"}
				class="w-full skeu-input text-[11px] font-mono min-h-[50px] resize-y mt-0.5"
				oninput={(e) => updateSettings('custom_cookies', (e.target as HTMLTextAreaElement).value)}></textarea>
			<p class="text-[9px] text-muted-foreground mt-0.5">One per line: name=value. Injected via CDP before navigation.</p>
		</div>
	</div>

<!-- ===================== CLICK ELEMENT ===================== -->
{:else if block.settings.type === 'ClickElement'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Selector</label>
			<VariableInput value={block.settings.selector} placeholder="#login-btn, .submit, button[type='submit']" class={inputCls}
				oninput={(e) => updateSettings('selector', (e.target as HTMLInputElement).value)} />
		</div>
		<label class="flex items-center gap-2 text-[11px] text-foreground">
			<input type="checkbox" checked={block.settings.wait_for_navigation} onchange={() => updateSettings('wait_for_navigation', !block!.settings.wait_for_navigation)} class="skeu-checkbox" />
			Wait for navigation after click
		</label>
		<div class="flex items-center gap-2">
			<span class="text-[11px] text-muted-foreground">Timeout:</span>
			<input type="number" value={block.settings.timeout_ms}
				class="w-20 skeu-input text-[11px]"
				oninput={(e) => updateSettings('timeout_ms', parseInt((e.target as HTMLInputElement).value))} />
			<span class="text-[10px] text-muted-foreground">ms</span>
		</div>
	</div>

<!-- ===================== TYPE TEXT ===================== -->
{:else if block.settings.type === 'TypeText'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Selector</label>
			<VariableInput value={block.settings.selector} placeholder="#username, input[name='email']" class={inputCls}
				oninput={(e) => updateSettings('selector', (e.target as HTMLInputElement).value)} />
		</div>
		<div>
			<label class={labelCls}>Text</label>
			<VariableInput value={block.settings.text} placeholder="Text to type (supports <variables>)" class={inputCls}
				oninput={(e) => updateSettings('text', (e.target as HTMLInputElement).value)} />
		</div>
		<label class="flex items-center gap-2 text-[11px] text-foreground">
			<input type="checkbox" checked={block.settings.clear_first} onchange={() => updateSettings('clear_first', !block!.settings.clear_first)} class="skeu-checkbox" />
			Clear field before typing
		</label>
		<div class="flex items-center gap-2">
			<span class="text-[11px] text-muted-foreground">Key delay:</span>
			<input type="number" value={block.settings.delay_ms}
				class="w-20 skeu-input text-[11px]"
				oninput={(e) => updateSettings('delay_ms', parseInt((e.target as HTMLInputElement).value))} />
			<span class="text-[10px] text-muted-foreground">ms</span>
		</div>
	</div>

<!-- ===================== WAIT FOR ELEMENT ===================== -->
{:else if block.settings.type === 'WaitForElement'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Selector</label>
			<VariableInput value={block.settings.selector} placeholder="#result, .loaded" class={inputCls}
				oninput={(e) => updateSettings('selector', (e.target as HTMLInputElement).value)} />
		</div>
		<div>
			<label class={labelCls}>Wait for state</label>
			<SkeuSelect value={block.settings.state}
				onValueChange={(v) => updateSettings('state', v)}
				options={[{value:'visible',label:'Visible'},{value:'hidden',label:'Hidden'},{value:'attached',label:'Attached'},{value:'detached',label:'Detached'}]}
				class="w-full mt-0.5"
			/>
		</div>
		<div class="flex items-center gap-2">
			<span class="text-[11px] text-muted-foreground">Timeout:</span>
			<input type="number" value={block.settings.timeout_ms}
				class="w-20 skeu-input text-[11px]"
				oninput={(e) => updateSettings('timeout_ms', parseInt((e.target as HTMLInputElement).value))} />
			<span class="text-[10px] text-muted-foreground">ms</span>
		</div>
	</div>

<!-- ===================== GET ELEMENT TEXT ===================== -->
{:else if block.settings.type === 'GetElementText'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Selector</label>
			<VariableInput value={block.settings.selector} placeholder="h1.title, #message" class={inputCls}
				oninput={(e) => updateSettings('selector', (e.target as HTMLInputElement).value)} />
		</div>
		<div>
			<label class={labelCls}>Attribute (empty = text content)</label>
			<VariableInput value={block.settings.attribute} placeholder="href, src, value" class={inputCls}
				oninput={(e) => updateSettings('attribute', (e.target as HTMLInputElement).value)} />
		</div>
		<div class="flex gap-2">
			<div class="flex-1">
				<label class={labelCls}>Output var</label>
				<VariableInput value={block.settings.output_var} class={inputCls}
					oninput={(e) => updateSettings('output_var', (e.target as HTMLInputElement).value)} />
			</div>
			<label class="flex items-center gap-1 text-xs text-foreground pt-4">
				<input type="checkbox" checked={block.settings.capture} onchange={() => updateSettings('capture', !block!.settings.capture)} class="skeu-checkbox" />
				CAP
			</label>
		</div>
	</div>

<!-- ===================== GET DOM ===================== -->
{:else if block.settings.type === 'GetDom'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Selector (optional)</label>
			<VariableInput value={block.settings.selector} placeholder="article, #content (empty = full page)" class={inputCls}
				oninput={(e) => updateSettings('selector', (e.target as HTMLInputElement).value)} />
			<p class="text-[9px] text-muted-foreground mt-0.5">Empty reads the current full page DOM. A selector reads one matching element.</p>
		</div>
		<label class="flex items-center gap-2 text-[11px] text-foreground">
			<input type="checkbox" checked={block.settings.outer_html} onchange={() => updateSettings('outer_html', !block!.settings.outer_html)} class="skeu-checkbox" />
			Include selected element
		</label>
		<div class="flex gap-2">
			<div class="flex-1">
				<label class={labelCls}>Output var</label>
				<VariableInput value={block.settings.output_var} class={inputCls}
					oninput={(e) => updateSettings('output_var', (e.target as HTMLInputElement).value)} />
			</div>
			<label class="flex items-center gap-1 text-xs text-foreground pt-4">
				<input type="checkbox" checked={block.settings.capture} onchange={() => updateSettings('capture', !block!.settings.capture)} class="skeu-checkbox" />
				CAP
			</label>
		</div>
	</div>

<!-- ===================== SCREENSHOT ===================== -->
{:else if block.settings.type === 'Screenshot'}
	<div class="space-y-1.5">
		<label class="flex items-center gap-2 text-[11px] text-foreground">
			<input type="checkbox" checked={block.settings.full_page} onchange={() => updateSettings('full_page', !block!.settings.full_page)} class="skeu-checkbox" />
			Full page screenshot
		</label>
		<div>
			<label class={labelCls}>Selector (optional, for element screenshot)</label>
			<VariableInput value={block.settings.selector} placeholder="#element" class={inputCls}
				oninput={(e) => updateSettings('selector', (e.target as HTMLInputElement).value)} />
		</div>
		<div>
			<label class={labelCls}>Output var (base64)</label>
			<VariableInput value={block.settings.output_var} class={inputCls}
				oninput={(e) => updateSettings('output_var', (e.target as HTMLInputElement).value)} />
		</div>
	</div>

<!-- ===================== EXECUTE JS ===================== -->
{:else if block.settings.type === 'ExecuteJs'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>JavaScript code</label>
			<textarea value={block.settings.code} placeholder="// Runs in browser context&#10;return document.title;"
				class="w-full skeu-input text-[11px] font-mono min-h-[120px] resize-y mt-0.5"
				oninput={(e) => updateSettings('code', (e.target as HTMLTextAreaElement).value)}></textarea>
			<p class="text-[9px] text-muted-foreground mt-0.5">Executes in the browser page context. Use <code class="text-foreground/70">return</code> to capture a value.</p>
		</div>
		<div class="flex gap-2">
			<div class="flex-1">
				<label class={labelCls}>Output var</label>
				<VariableInput value={block.settings.output_var} class={inputCls}
					oninput={(e) => updateSettings('output_var', (e.target as HTMLInputElement).value)} />
			</div>
			<label class="flex items-center gap-1 text-xs text-foreground pt-4">
				<input type="checkbox" checked={block.settings.capture} onchange={() => updateSettings('capture', !block!.settings.capture)} class="skeu-checkbox" />
				CAP
			</label>
		</div>
	</div>
{/if}
