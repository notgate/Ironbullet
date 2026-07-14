import type { BlockType } from './pipeline';

/** Single live check result from the runner's ring buffer. */
export interface ResultEntry {
	data_line: string;
	/** "SUCCESS" | "FAIL" | "BAN" | "RETRY" | "ERROR" | "NONE" */
	status: string;
	proxy?: string | null;
	captures: Record<string, string>;
	error?: string | null;
	ts_ms: number;
	block_results: BlockResult[];
}

export interface RunnerStats {
	total: number;
	processed: number;
	consumed: number;
	hits: number;
	fails: number;
	bans: number;
	retries: number;
	errors: number;
	to_check: number;
	cpm: number;
	active_threads: number;
	elapsed_secs: number;
	/** Last ≤100 live results from the ring buffer. Empty when idle. */
	recent_results: ResultEntry[];
}

export interface BlockResult {
	block_id: string;
	block_label: string;
	block_type: BlockType;
	success: boolean;
	timing_ms: number;
	variables_after: Record<string, string>;
	log_message: string;
	request?: RequestInfo;
	response?: ResponseInfo;
}

export interface RequestInfo {
	method: string;
	url: string;
	headers: [string, string][];
	body: string;
}

export interface ResponseInfo {
	status_code: number;
	headers: Record<string, string>;
	body: string;
	final_url: string;
	cookies: Record<string, string>;
	timing_ms: number;
}

export interface NetworkEntry {
	block_id: string;
	block_label: string;
	method: string;
	url: string;
	status_code: number;
	timing_ms: number;
	response_size: number;
	cookies_set: [string, string][];
	cookies_sent: [string, string][];
}
