import { app, loadPipelineIntoTab, markTabSaved, openInNewTab, takePipelineSnapshot } from './state.svelte';
import { toast } from './toast.svelte';

const MUTATION_CMDS = new Set([
	'add_block', 'remove_block', 'move_block', 'add_block_nested',
	'move_block_to_nested', 'update_block', 'remove_blocks', 'toggle_blocks', 'paste_blocks',
	'move_blocks_to', 'group_blocks',
]);

// High-frequency cmds to suppress from send() log
const SILENT_SEND_CMDS = new Set(['get_runner_stats']);

/** Save the active pipeline, passing its current tab file path so the backend
 *  never uses a stale path from a previously opened config. */
export function savePipeline(opts: { forceDialog?: boolean } = {}) {
	const tab = app.configTabs.find((t: any) => t.id === app.activeTabId) as any;
	const filePath = tab?.filePath ?? null;
	send('save_pipeline', {
		...(filePath ? { path: filePath } : {}),
		...(opts.forceDialog ? { force_dialog: true } : {}),
	});
}

// Commands that must be allowed through even from panel windows.
// These are panel-local actions that don't cause cross-window broadcast issues.
const PANEL_ALLOWED_CMDS = new Set([
	'inspect_browser_open',
	'inspect_browser_close',
]);

export function send(cmd: string, data: Record<string, unknown> = {}) {
	// In native panel windows this webview is receive-only.
	// Sending IPC from panel windows causes Rust to broadcast responses to ALL
	// webviews — including the main window — re-triggering startup dialogs.
	// Exception: panel-local commands (browser capture) must still be sent.
	if (typeof window !== 'undefined' && (window as any).__ibIsPanelMode && !PANEL_ALLOWED_CMDS.has(cmd)) return;

	const payload: Record<string, unknown> = { ...data, _tab_id: app.activeTabId };
	if (MUTATION_CMDS.has(cmd)) {
		// Send current blocks so Rust syncs before mutating (avoids cross-tab bleed)
		payload._blocks = JSON.parse(JSON.stringify(app.pipeline.blocks));
		payload._startup_blocks = JSON.parse(JSON.stringify(app.pipeline.startup_blocks));
	}
	if (!SILENT_SEND_CMDS.has(cmd)) {
		// Log outgoing IPC (omit large _blocks payload for readability)
		const logData: Record<string, unknown> = {};
		for (const [k, v] of Object.entries(payload)) {
			if (k === '_blocks' || k === '_startup_blocks') { logData[k] = `[${Array.isArray(v) ? (v as unknown[]).length : '?'} blocks]`; }
			else { logData[k] = v; }
		}
		console.log(`[IPC] → ${cmd}`, logData);
	}
	window.ipc.postMessage(JSON.stringify({ cmd, data: payload }));
}

/** One-shot IPC call that resolves when the backend responds with the matching event. */
const pendingCallbacks = new Map<string, (data: any) => void>();
export function sendAsync(cmd: string, responseEvent: string, data: Record<string, unknown> = {}): Promise<any> {
	return new Promise((resolve) => {
		pendingCallbacks.set(responseEvent, resolve);
		send(cmd, data);
		// Safety timeout so we never hang
		setTimeout(() => { if (pendingCallbacks.delete(responseEvent)) resolve(null); }, 3000);
	});
}
// Called from registerCallbacks — resolves pending async calls
export function _resolvePending(event: string, data: any): boolean {
	const cb = pendingCallbacks.get(event);
	if (cb) { pendingCallbacks.delete(event); cb(data); return true; }
	return false;
}

export function saveSettings() {
    send('save_config', {
        zoom: Math.round(app.zoom * 100),
        font_size: app.fontSize,
        font_family: app.fontFamily,
        font_weight: app.fontWeight,
        default_threads: app.pipeline.runner_settings.threads,
        left_panel_width: app.leftPanelWidth,
        bottom_panel_height: app.bottomPanelHeight,
        show_block_palette: app.showBlockPalette,
        collections_path: app.collectionsPath,
        default_wordlist_path: app.defaultWordlistPath,
        default_proxy_path: app.defaultProxyPath,
        configs_path: app.configsPath,
        results_path: app.resultsPath,
        chrome_executable_path: (app.config as any)?.chrome_executable_path || '',
        plugins_path: (app.config as any)?.plugins_path || '',
    });
}

// Callback registry for async IPC responses
const callbacks: Record<string, (data: unknown) => void> = {};

export function onResponse(cmd: string, callback: (data: unknown) => void) {
	callbacks[cmd] = callback;
}

export function registerCallbacks() {
	// Global IPC callback handler invoked by Rust via eval_js
	(window as any).__ipc_callback = (resp: { cmd: string; success: boolean; data?: unknown; error?: string }) => {
		// Debug: log every IPC response (suppress high-frequency polling cmds)
		const SILENT_CMDS = new Set(['runner_stats']);
		if (!SILENT_CMDS.has(resp.cmd)) {
			console.log(`[IPC] ← ${resp.cmd}`, resp.success ? 'OK' : `ERR: ${resp.error}`, resp.data !== undefined ? resp.data : '');
		}

		// Resolve pending sendAsync calls first
		if (_resolvePending(resp.cmd, resp.data)) return;

		// Route to specific handler
		switch (resp.cmd) {
			case 'config_loaded':
				if (resp.data) {
					const cfg = resp.data as any;

					// Startup integrity check — show missing-dependency warnings.
					const startupIssues = cfg._security_issues as Array<{ severity: string; title: string; description: string; code_snippet: string }> | undefined;
					if (startupIssues && startupIssues.length > 0) {
						// Only show warnings if user hasn't opted out
						const allWarnings = startupIssues.every(i => i.severity === 'Warning');
						if (!allWarnings || !app.uiPrefs.skipDependencyWarnings) {
							app.securityIssues = startupIssues;
						}
					}

					app.config = cfg;
					if (cfg.zoom) app.zoom = cfg.zoom / 100;
					if (cfg.font_size) app.fontSize = cfg.font_size;
					if (cfg.font_family) app.fontFamily = cfg.font_family;
					if (cfg.font_weight) app.fontWeight = cfg.font_weight;
					if (cfg.default_threads) app.threadCount = cfg.default_threads;
					if (cfg.left_panel_width) app.leftPanelWidth = cfg.left_panel_width;
					if (cfg.bottom_panel_height) app.bottomPanelHeight = cfg.bottom_panel_height;
					if (cfg.show_block_palette !== undefined) app.showBlockPalette = cfg.show_block_palette;
if (cfg.collections_path) app.collectionsPath = cfg.collections_path;
                    if (cfg.default_wordlist_path) app.defaultWordlistPath = cfg.default_wordlist_path;
                    if (cfg.default_proxy_path) app.defaultProxyPath = cfg.default_proxy_path;
                    if (cfg.configs_path) app.configsPath = cfg.configs_path;
                    if (cfg.results_path) app.resultsPath = cfg.results_path;
                }
				break;
			case 'pipeline_loaded':
				if (resp.data) {
					const raw = resp.data as any;
					const loadedPath = raw._file_path as string | undefined;
					const tabId = raw._tab_id as string | undefined;
					// Remove internal fields before setting pipeline
					if (raw._file_path !== undefined) delete raw._file_path;
					if (raw._tab_id !== undefined) delete raw._tab_id;
					if (loadedPath) {
						// File load — open in new tab with full pipeline.
						// On startup (get_pipeline), the first pipeline_loaded with a file
						// path means a previously-saved config was restored — load it into
						// the existing blank tab instead of opening an extra one, and hide
						// the startup dialog so the user goes straight to their workspace.
						const isStartupRestore = app.configTabs.length === 1
							&& !app.configTabs[0].filePath
							&& (app.configTabs[0].pipeline?.blocks?.length ?? 0) === 0;
						if (isStartupRestore) {
							loadPipelineIntoTab(raw, loadedPath);
							app.pipeline = JSON.parse(JSON.stringify(raw));
							app.showStartup = false;
						} else {
							openInNewTab(raw, loadedPath);
						}
						setTimeout(takePipelineSnapshot, 50);
					} else if (tabId) {
						// Mutation response — only update blocks, preserve name/settings
						if (tabId === app.activeTabId) {
							app.pipeline.blocks = raw.blocks || [];
							if (raw.startup_blocks) app.pipeline.startup_blocks = raw.startup_blocks;
							const tab = app.configTabs.find((t: any) => t.id === tabId);
							if (tab) {
								tab.pipeline = JSON.parse(JSON.stringify(app.pipeline));
							}
						} else {
							// Response for a different tab — update only that tab's stored blocks
							const tab = app.configTabs.find((t: any) => t.id === tabId);
							if (tab) {
								tab.pipeline.blocks = raw.blocks || [];
								if (raw.startup_blocks) tab.pipeline.startup_blocks = raw.startup_blocks;
							}
						}
					} else {
						// Initial load or import — full replacement
						const importWarnings = raw._import_warnings as string[] | undefined;
						const securityIssues = raw._security_issues as Array<{ severity: string; title: string; description: string; code_snippet: string }> | undefined;
						if (raw._import_warnings !== undefined) delete raw._import_warnings;
						if (raw._security_issues !== undefined) delete raw._security_issues;
						loadPipelineIntoTab(raw);
						app.pipeline = raw;
						setTimeout(takePipelineSnapshot, 50);
						if (importWarnings && importWarnings.length > 0) {
							toast(`Import completed with notes: ${importWarnings.join('; ')}`, 'warning');
						}
						if (securityIssues && securityIssues.length > 0) {
							app.securityIssues = securityIssues;
						}
					}
				}
				break;
			case 'pipeline_updated':
				console.log('[IPC] pipeline_updated: backend state synced');
				break;
			case 'dirs_created': {
				const dc = resp.data as { created: string[]; paths: Record<string, string> } | null;
				if (dc) {
					console.log('[IPC] dirs_created:', dc.created.length, 'created, paths:', dc.paths);
					app.setupDirsDone = true;
					app.setupDirsPaths = dc.paths;
					// Pre-fill default paths if not already set
					if (!app.wordlistPath && dc.paths['wordlists']) app.wordlistPath = dc.paths['wordlists'];
					if (!app.proxyPath && dc.paths['proxies']) app.proxyPath = dc.paths['proxies'];
				}
				break;
			}
			case 'code_generated':
				if (resp.data) {
					app.generatedCode = (resp.data as any).code || '';
				}
				break;
			case 'runner_stats':
				if (resp.data) app.runnerStats = resp.data as any;
				break;
			case 'code_saved':
				if (resp.success) {
					toast(`Code saved: ${(resp.data as any)?.path || ''}`, 'success');
				} else {
					toast(`Save failed: ${resp.error}`, 'error');
				}
				break;
			case 'pipeline_saved':
				if (resp.success) {
					const savedPath = (resp.data as any)?.path || '';
					app.statusText = `Config saved: ${savedPath}`;
					markTabSaved(savedPath || undefined);
					takePipelineSnapshot();
					toast('Config saved', 'success');
				} else {
					app.statusText = `Save error: ${resp.error}`;
					toast(`Save failed: ${resp.error}`, 'error');
				}
				break;
			case 'debug_step':
				if (resp.data) app.debugResult = resp.data as any;
				break;
			case 'debug_results':
				if (resp.data) {
					app.debugResults = resp.data as any;
					app.showResponseViewer = true;
					toast('Debug complete', 'success');
				}
				break;
			case 'network_log':
				if (resp.data) app.networkLog = resp.data as any;
				break;
			case 'debug_error':
				toast(`Debug error: ${resp.error || 'Unknown error'}`, 'error');
				break;
			case 'runner_started':
				console.log('[IPC] runner_started: setting isRunning=true');
				app.isRunning = true;
				toast('Runner started', 'info');
				break;
			case 'runner_stopped':
				console.log('[IPC] runner_stopped: setting isRunning=false, isPaused=false');
				toast('Runner stopped', 'info');
				app.isRunning = false;
				app.isPaused = false;
				break;
			case 'runner_error':
				console.error('[IPC] runner_error:', resp.error);
				toast(`Runner error: ${resp.error || 'Unknown error'}`, 'error');
				app.isRunning = false;
				app.isPaused = false;
				break;
			case 'proxy_check_result':
				if (resp.data) {
					const r = resp.data as { alive: number; dead: number; total: number };
					toast(`Proxies: ${r.alive}/${r.total} alive (${r.dead} dead)`, r.alive > 0 ? 'success' : 'warning');
				}
				break;
			case 'file_selected':
				if (resp.data) {
					const { field, path } = resp.data as { field: string; path: string };
					console.log('[IPC] file_selected: field=', field, 'path=', path);
					if (field === 'wordlist') app.wordlistPath = path;
					else if (field === 'proxies') app.proxyPath = path;
					else if (field === 'job_wordlist' || field === 'proxy_check_list') app.pendingJobWordlist = { path, isFolder: false };
					else if (field === 'job_proxy_file') app.pendingJobProxyFile = path;
					else if (field === 'job_config') app.pendingJobConfig = path;
					else if (field === 'chrome_exe') {
						(app.config as any).chrome_executable_path = path;
						saveSettings();
					} else if (field === 'proxy_group_source') {
						// One-shot callback registered by browseGroupSourceFile() in ProxiesSection
						const cb = (window as any).__proxyGroupFilePicked;
						if (typeof cb === 'function') cb(path);
					}
				}
				break;
			case 'recent_configs':
				if (resp.data && Array.isArray(resp.data)) {
					app.recentConfigs = resp.data as any;
					app.showStartup = true;
				} else {
					app.recentConfigs = [];
					app.showStartup = true;
				}
				break;
			case 'collections_list':
				if (resp.data && Array.isArray(resp.data)) {
					app.collectionConfigs = resp.data as any;
				}
				break;
			case 'configs_list':
				if (resp.data && Array.isArray(resp.data)) {
					app.configsList = resp.data as any;
				}
				break;
			case 'folder_selected':
				if (resp.data) {
					const { field: folderField, path: folderPath } = resp.data as { field: string; path: string };
					console.log('[IPC] folder_selected: field=', folderField, 'path=', folderPath);
					if (folderField === 'collections') app.collectionsPath = folderPath;
					else if (folderField === 'wordlist_dir') app.defaultWordlistPath = folderPath;
					else if (folderField === 'proxy_dir') app.defaultProxyPath = folderPath;
					else if (folderField === 'plugins') (app.config as any).plugins_path = folderPath;
					else if (folderField === 'job_folder') app.pendingJobWordlist = { path: folderPath, isFolder: true };
					else if (folderField === 'results') app.pipeline.output_settings.output_directory = folderPath;
				}
				break;
			case 'import_success':
				toast('Config imported successfully', 'success');
				break;
			case 'import_error':
				toast(`Import failed: ${resp.error || 'Unknown'}`, 'error');
				break;
			case 'job_start_error':
				toast(resp.error || 'Failed to start job', 'error');
				break;
			case 'job_preflight_error':
				// Missing wordlist/proxy files — show prominent error before job starts
				toast(resp.error || 'Job cannot start: missing files', 'error');
				break;
			case 'autosave_available':
				// Crash recovery — offer to restore unsaved pipeline
				if (resp.data) {
					const autosavePath = (resp.data as any).path as string;
					if (confirm('IronBullet found an unsaved session from a previous run.\n\nRestore it?')) {
						send('load_pipeline', { path: autosavePath });
					} else {
						send('delete_autosave', {});
					}
				}
				break;
			case 'job_created':
				if (resp.data) {
					console.log('[IPC] job_created:', (resp.data as any)?.id ?? 'no-id');
					toast('Job created', 'success');
					send('list_jobs');
				}
				break;
			case 'job_updated':
				console.log('[IPC] job_updated:', (resp.data as any)?.id ?? 'no-id');
				toast('Job updated', 'success');
				send('list_jobs');
				break;
			case 'jobs_list':
				if (resp.data && Array.isArray(resp.data)) {
					const newJobsList = resp.data as any[];
					const liveJobIds = new Set(newJobsList.map((j: any) => j.id));

					// Prune jobHitsDb entries for jobs that no longer exist
					const currentDb = app.jobHitsDb;
					const staleIds = Object.keys(currentDb).filter(id => !liveJobIds.has(id));
					if (staleIds.length > 0) {
						const pruned: typeof currentDb = {};
						for (const [k, v] of Object.entries(currentDb)) {
							if (liveJobIds.has(k)) pruned[k] = v;
						}
						app.jobHitsDb = pruned;
					}

					// Merge into existing array to avoid full re-render flicker.
					// Update fields in-place for existing jobs; append new ones; remove gone ones.
					const existingIds = new Set((app.jobs as any[]).map((j: any) => j.id));
					for (const incoming of newJobsList) {
						const existing = (app.jobs as any[]).find((j: any) => j.id === incoming.id);
						if (existing) {
							// Merge all fields in-place — Svelte 5 proxy tracks property assignments
							for (const key of Object.keys(incoming)) {
								if (JSON.stringify(existing[key]) !== JSON.stringify(incoming[key])) {
									existing[key] = incoming[key];
								}
							}
						} else {
							(app.jobs as any[]).push(incoming);
						}
					}
					// Remove jobs that disappeared
					const toRemove = (app.jobs as any[]).filter((j: any) => !liveJobIds.has(j.id));
					for (const gone of toRemove) {
						const idx = (app.jobs as any[]).indexOf(gone);
						if (idx !== -1) (app.jobs as any[]).splice(idx, 1);
					}
				}
				break;
			case 'job_stats_update':
				if (resp.data) {
					const { id: jobId, stats: jobStats } = resp.data as any;
					console.log(`[IPC] job_stats_update: job ${jobId}`, jobStats);
					const job = app.jobs.find((j: any) => j.id === jobId);
					if (job && jobStats) (job as any).stats = jobStats;
				}
				break;
			case 'runner_hit':
				if (resp.data) {
					const hit = resp.data as { job_id?: string; data_line: string; captures: Record<string, string>; proxy: string | null };
					const stamped = { data_line: hit.data_line, captures: hit.captures, proxy: hit.proxy, received_at: new Date().toISOString() };
					// Only push REAL hits (alive proxies / config job hits) to the hits DB.
					// Dead (status=dead) and error (status=error) proxy results are discarded here
					// — they show in the runner stats counters (Fails/Errors) but not in the DB.
					const status = hit.captures?.status;
					const isActualHit = !status || status === 'alive';
					if (isActualHit) {
						// Global hits feed (used by HitsDialog "All Jobs" view)
						// Drop oldest 500 when we reach 5 000 to stay under the render budget.
						if (app.hits.length >= 5000) (app.hits as any[]).splice(0, 500);
						(app.hits as any[]).push(stamped);
						// Per-job hits DB — keyed by job_id for the per-job dropdown view
						if (hit.job_id) {
							const db = app.jobHitsDb;
							if (!db[hit.job_id]) {
								// Svelte 5: assign a new object to trigger reactivity on first insert
								app.jobHitsDb = { ...db, [hit.job_id]: [stamped] };
							} else {
								// In-place push — Svelte 5 proxy tracks array mutations
								if (db[hit.job_id].length >= 10000) (db[hit.job_id] as any[]).splice(0, 500);
								(db[hit.job_id] as any[]).push(stamped);
							}
						}
					}
				}
				break;
			case 'job_hits': {
				// Backend returns { id: string, hits: HitResult[] }
				// Used for Hits Database panel: on-demand fetch / manual refresh.
				// Replaces the jobHitsDb entry for this job with the full snapshot from the backend.
				const jobHitsPayload = resp.data as { id: string; hits: Array<{ data_line: string; captures: Record<string, string>; proxy: string | null }> } | null;
				if (jobHitsPayload && Array.isArray(jobHitsPayload.hits)) {
					const { id: jobId, hits: incoming } = jobHitsPayload;
					const stamped = incoming.map((h: any) => ({ ...h, received_at: h.received_at ?? new Date().toISOString() }));
					// Full replace — this is a fresh snapshot from the DB, not an incremental append.
					app.jobHitsDb = { ...app.jobHitsDb, [jobId]: stamped };
					console.log(`[IPC] job_hits: loaded ${stamped.length} hits for job ${jobId}`);
				} else {
					console.warn('[IPC] job_hits: unexpected payload shape', resp.data);
				}
				break;
			}
			case 'plugin_blocks_loaded':
				if (resp.data) {
					const pd = resp.data as any;
					if (pd.blocks) app.pluginBlocks = pd.blocks;
					if (pd.plugins) app.pluginMetas = pd.plugins;
				}
				break;
			case 'compile_output':
				if (resp.data && app._compileOutputCallback) {
					app._compileOutputCallback(resp.data as any);
				}
				break;
			case 'save_plugin_result':
				if (resp.success && resp.data) {
					toast(`Plugin saved to: ${(resp.data as any).dir}`, 'success');
				}
				break;
			case 'app_info':
				if (resp.data) {
					app.updateCurrentVersion = (resp.data as any).version || '0.1.0';
				}
				break;
			case 'update_check_result':
				app.updateChecking = false;
				if (resp.data) {
					const ud = resp.data as any;
					app.updateAvailable = ud.has_update || false;
					app.updateLatestVersion = ud.latest_version || '';
					app.updateCurrentVersion = ud.current_version || '0.1.0';
					app.updateReleaseNotes = ud.release_notes || '';
					app.updateDownloadUrl = ud.download_url || '';
					app.updatePublishedAt = ud.published_at || '';
					if (ud.has_update) {
						app.showUpdateDialog = true;
					}
				}
				break;
			case 'update_progress':
				if (resp.data) {
					app.updateProgress = (resp.data as any).percent || 0;
				}
				break;
			case 'probe_result':
				// Handled by registered callback in FingerprintDialog
				break;
			case 'update_download_result':
				if (resp.success) {
					app.updateInstalling = false;
					app.updateComplete = true;
					toast('Update installed — restart to apply', 'success');
				} else {
					app.updateInstalling = false;
					toast(`Update failed: ${resp.error}`, 'error');
				}
				break;
		}

		// Generic error toast for any failed command
		if (!resp.success && resp.error && !['pipeline_saved', 'debug_error', 'runner_error', 'import_error'].includes(resp.cmd)) {
			toast(`${resp.cmd}: ${resp.error}`, 'warning');
		}

		// Also call registered callback if any
		if (callbacks[resp.cmd]) {
			callbacks[resp.cmd](resp.data);
		}
	};

	// Request initial state
	send('get_app_info');
	send('get_config');
	send('get_pipeline');
	send('get_recent_configs');
	send('get_plugin_blocks');

	// Auto-check for updates after a short delay
	setTimeout(() => send('check_for_updates'), 3000);
}
