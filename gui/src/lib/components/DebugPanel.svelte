<script lang="ts">
	import { app } from '$lib/state.svelte';
	import { send } from '$lib/ipc';
	import { getBlockColor } from '$lib/types';
	import type { BlockResult } from '$lib/types';
	import Play from '@lucide/svelte/icons/play';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import CircleX from '@lucide/svelte/icons/circle-x';
	import ExternalLink from '@lucide/svelte/icons/external-link';
	import HelpCircle from '@lucide/svelte/icons/help-circle';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Copy from '@lucide/svelte/icons/copy';
	import HelpModal from './HelpModal.svelte';

	let testDataLine = $state('user@example.com:pass123');
	let showHelp = $state(false);

	const helpSections = [
		{
			heading: 'What is Debug Mode?',
			content: `Debug mode allows you to test your pipeline with a single data line before processing full datasets. This is a dry run to verify pipeline logic, inspect variables, and catch errors early.

Benefits:
- Test pipeline logic without processing thousands of entries
- Inspect variable values at each execution step
- Identify configuration errors before running jobs
- Verify HTTP requests and responses in detail
- Review generated Rust code for each block`
		},
		{
			heading: 'Quick Start',
			content: `1. Enter Test Data
In the "Data" field, provide a single test entry matching your wordlist format:
  user123:pass456              (username:password)
  test@example.com:mypass      (email:password)
  custom|format|here           (custom separator)

2. Add Proxy (Optional)
Standard format: http://ip:port or http://user:pass@ip:port
Encrypted format: vmess://, vless://, or trojan://
For encrypted URIs, Ironbullet starts the bundled Xray Core and uses its private local SOCKS5 listener.

3. Execute
Click "Debug Run" button or press F5 to run the pipeline once

4. Review Results
Check the block timeline table below for execution status and timing`
		},
		{
			heading: 'Understanding Results',
			content: `Block Timeline Columns:

# (Number)
  Block execution order in the pipeline

Block
  Block type with color-coded indicator

Result
  Output message, extracted data, or error details

Time
  Execution duration in milliseconds

Status Icons:
  Green checkmark - Block executed successfully
  Red X - Block failed or encountered an error
  Link icon - HTTP response data available

Interaction:
- Click any row to open detailed views
- Left panel: HTTP request/response details
- Right panel: Variable Inspector with all captured data
- Terminal output: Generated Rust code showing execution logic`
		},
		{
			heading: 'Debugging Workflow',
			content: `Recommended approach: Start simple, add complexity incrementally

1. Test with known-good credentials first
2. Add blocks one at a time to isolate issues
3. Verify each block's output before proceeding

Common debugging scenarios:

Block fails
  → Check Result column for specific error message
  → Review terminal output for stack traces

Wrong data extracted
  → Open Variable Inspector (right panel)
  → Verify selector patterns (CSS, Regex, JSON path)

HTTP errors
  → Click block row to view full request/response
  → Check status codes, headers, cookies

Logic errors
  → Review Rust code view in terminal
  → Verify variable interpolation is correct

Configuration tips:
- Enable safe mode on optional blocks to continue on failure
- Temporarily disable blocks to isolate problems
- Variable names are case-sensitive (USER vs user)`
		},
		{
			heading: 'Pre-Job Validation',
			content: `Always run debug mode before creating jobs with full datasets.

Success criteria checklist:

[ ] All blocks show green checkmarks
[ ] Variable Inspector contains expected extracted data
[ ] HTTP blocks return appropriate status codes (200, 302, etc.)
[ ] KeyCheck conditions evaluate correctly
[ ] Final pipeline output matches success criteria
[ ] No errors in terminal output

Common validations:

Authentication flows
  Verify login requests succeed and session cookies are captured

Data extraction
  Check that parsers extract correct values from responses

Variable usage
  Confirm variables are interpolated correctly in subsequent blocks

Error handling
  Test with invalid credentials to verify FAIL status is set

Once all validations pass, you can create a job with confidence that the pipeline will execute correctly at scale.`
		},
	];
	let testProxy = $state('');
	let encryptedDebugProxy = $derived(/^(vmess|vless|trojan):\/\//i.test(testProxy.trim()));

	// Sync test inputs to app state so ContextMenu "Debug Block" can read them
	$effect(() => { app.debugTestDataLine = testDataLine; });
	$effect(() => { app.debugTestProxy = testProxy; });

	// When ContextMenu sets debugBlockIds, auto-trigger a debug run for just those blocks
	$effect(() => {
		const ids = app.debugBlockIds;
		if (ids !== null && ids.length > 0) {
			console.log('[DebugPanel] debugBlockIds triggered:', ids);
			app.debugBlockIds = null; // consume before running to avoid re-trigger
			send('debug_pipeline', {
				test_data_line: testDataLine,
				test_proxy: testProxy || null,
				pipeline: JSON.parse(JSON.stringify(app.pipeline)),
				block_ids: ids,
			});
		}
	});

	function runDebug() {
		console.log('[DebugPanel] debug_pipeline: data="%s" proxy="%s"', testDataLine, testProxy || 'none');
		send('debug_pipeline', {
			test_data_line: testDataLine,
			test_proxy: testProxy || null,
			pipeline: JSON.parse(JSON.stringify(app.pipeline)),
		});
	}

	let results = $derived(app.debugResults);
	let hasResults = $derived(results.length > 0);

	/** Which block rows have their header panel expanded */
	let expandedRows = $state<Set<number>>(new Set());

	function toggleExpand(i: number) {
		const next = new Set(expandedRows);
		if (next.has(i)) next.delete(i); else next.add(i);
		expandedRows = next;
	}

	/** Open the response viewer only when the user clicked (not selected text) */
	function handleRowClick(e: MouseEvent, index: number) {
		if (window.getSelection()?.toString()) return; // text was selected — don't open viewer
		const r = results[index];
		if (r?.response) app.showResponseViewer = true;
	}

	async function copyText(text: string) {
		try { await navigator.clipboard.writeText(text); } catch {}
	}

	function truncate(s: string, max: number): string {
		return s.length > max ? s.slice(0, max) + '...' : s;
	}
</script>

<div class="flex flex-col h-full bg-surface">
	<!-- Debug toolbar -->
	<div class="flex items-center gap-2 px-2 py-1 panel-raised flex-wrap">
		<button
			class="skeu-btn flex items-center gap-1 text-green text-[11px] shrink-0"
			onclick={runDebug}
		><Play size={11} />Debug Run</button>

		<button
			class="p-1 rounded hover:bg-accent/20 text-muted-foreground hover:text-foreground transition-colors"
			onclick={() => { showHelp = true; }}
			title="Help & Documentation"
		>
			<HelpCircle size={14} />
		</button>

		<div class="flex items-center gap-1 flex-1 min-w-0">
			<label class="text-[9px] uppercase tracking-wider text-muted-foreground shrink-0">Data:</label>
			<input
				type="text"
				bind:value={testDataLine}
				placeholder="user:pass"
				class="skeu-input text-[10px] font-mono flex-1 min-w-[120px] py-0"
			/>
		</div>

		<div class="flex items-center gap-1 min-w-0">
			<label class="text-[9px] uppercase tracking-wider text-muted-foreground shrink-0">Proxy:</label>
			<input
				type="text"
				bind:value={testProxy}
				placeholder="http://… or vmess://… (optional)"
				class="skeu-input text-[10px] font-mono w-[180px] py-0"
			/>
		</div>
		{#if encryptedDebugProxy}
			<span class="text-[9px] text-primary font-mono shrink-0" title="Debug Run will validate the bundled Xray listener before your pipeline request">XRAY → SOCKS5</span>
		{/if}
	</div>

	{#if hasResults}
		<!-- Block timeline table -->
		<div class="flex-1 overflow-auto panel-inset">
			<table class="w-full text-[11px]">
				<thead class="sticky top-0 bg-surface z-10">
					<tr class="border-b border-border text-[9px] uppercase tracking-wider text-muted-foreground">
						<th class="text-left px-2 py-1 w-7">#</th>
						<th class="text-left px-2 py-1 w-[140px]">Block</th>
						<th class="text-left px-2 py-1">Result</th>
						<th class="text-right px-2 py-1 w-16">Time</th>
						<th class="text-center px-2 py-1 w-8"></th>
					</tr>
				</thead>
				<tbody>
					{#each results as r, i}
						<!-- Main result row -->
						<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
						<tr
							class="border-b border-border/50 hover:bg-secondary/30 cursor-pointer transition-colors group"
							class:opacity-60={!r.success}
							onclick={(e) => handleRowClick(e, i)}
						>
							<td class="px-2 py-0.5 text-muted-foreground font-mono select-none">{i + 1}</td>
							<td class="px-2 py-0.5 select-none">
								<div class="flex items-center gap-1.5">
									<span class="w-2 h-2 rounded-full shrink-0" style="background-color: {getBlockColor(r.block_type)}"></span>
									<span class="text-foreground truncate">{r.block_label}</span>
								</div>
							</td>
							<!-- Result: select-text so log messages can be highlighted and copied -->
							<td class="px-2 py-0.5 font-mono text-muted-foreground max-w-0 select-text">
								<div class="flex items-center gap-1 min-w-0">
									<span class="truncate flex-1 min-w-0" title={r.log_message || ''}>{truncate(r.log_message || '', 80)}</span>
									{#if r.log_message}
										<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
										<span class="shrink-0 p-0.5 opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-foreground cursor-pointer"
											onclick={(e) => { e.stopPropagation(); copyText(r.log_message || ''); }}
											title="Copy result"><Copy size={9} /></span>
									{/if}
								</div>
							</td>
							<td class="px-2 py-0.5 text-right font-mono text-muted-foreground tabular-nums select-none">{r.timing_ms}ms</td>
							<td class="px-2 py-0.5 text-center select-none">
								<div class="flex items-center justify-center gap-0.5">
									{#if r.success}
										<CircleCheck size={12} class="text-green" />
									{:else}
										<CircleX size={12} class="text-red" />
									{/if}
									{#if r.response}
										<ExternalLink size={10} class="text-primary/50" />
										<!-- expand/collapse header panel -->
										<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
										<span class="cursor-pointer text-muted-foreground hover:text-foreground"
											onclick={(e) => { e.stopPropagation(); toggleExpand(i); }}
											title={expandedRows.has(i) ? 'Hide headers' : 'Show response headers'}>
											{#if expandedRows.has(i)}<ChevronDown size={10} />{:else}<ChevronRight size={10} />{/if}
										</span>
									{/if}
								</div>
							</td>
						</tr>

						<!-- Inline header capture panel -->
						{#if r.response && expandedRows.has(i)}
							<tr class="border-b border-border bg-background">
								<td colspan="5" class="px-3 py-2">
									<div class="text-[9px] uppercase tracking-wider text-muted-foreground mb-1.5 font-semibold">
										Response Headers &mdash; {r.block_label} <span class="{r.response.status_code < 400 ? 'text-green' : 'text-red'}">[{r.response.status_code}]</span>
									</div>
									<div class="space-y-0.5">
										{#each Object.entries(r.response.headers) as [key, value]}
											<div class="flex items-baseline gap-1 font-mono text-[10px] group/hdr select-text">
												<span class="text-primary shrink-0 w-[160px] truncate">{key}:</span>
												<span class="text-foreground break-all flex-1 min-w-0">{value}</span>
												<button class="shrink-0 p-0.5 opacity-0 group-hover/hdr:opacity-100 text-muted-foreground hover:text-foreground"
													onclick={() => copyText(`${key}: ${value}`)} title="Copy header"><Copy size={9} /></button>
											</div>
										{/each}
										{#if Object.keys(r.response.headers).length === 0}
											<div class="text-[10px] text-muted-foreground/50">No response headers captured</div>
										{/if}
									</div>
								</td>
							</tr>
						{/if}
					{/each}
				</tbody>
			</table>
		</div>
	{:else}
		<div class="flex items-center justify-center flex-1 text-muted-foreground text-xs panel-inset">
			Enter test credentials and click "Debug Run" to execute the pipeline once
		</div>
	{/if}
</div>

<HelpModal bind:open={showHelp} title="Debug Mode Guide" sections={helpSections} />
