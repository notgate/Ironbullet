import type { Pipeline, ProxySettings } from './pipeline';
import type { RunnerStats } from './runtime';

export type JobState = 'Queued' | 'Waiting' | 'Running' | 'Paused' | 'Completed' | 'Stopped';

export type DataSourceType = 'File' | 'Folder' | 'Url' | 'Inline' | 'Range' | 'Combinations';

export interface DataSource {
	source_type: DataSourceType;
	value: string;
}

export type StartCondition =
	| { Immediate: null }
	| { Delayed: { delay_secs: number } }
	| { Scheduled: { at: string } };

export type HitOutput =
	| { FileSystem: { directory: string; format: 'Txt' | 'Csv' | 'Json' } }
	| { DiscordWebhook: { webhook_url: string; template: string } }
	| { TelegramBot: { bot_token: string; chat_id: string; template: string } }
	| { CustomWebhook: { url: string; method: string; body_template: string } };

export interface Job {
	id: string;
	name: string;
	config_path: string | null;
	pipeline: Pipeline;
	data_source: DataSource;
	proxy_source: { settings: ProxySettings };
	state: JobState;
	start_condition: StartCondition;
	hit_outputs: HitOutput[];
	thread_count: number;
	created: string;
	started: string | null;
	completed: string | null;
	stats: RunnerStats;
	job_type: 'Config' | 'ProxyCheck';
	proxy_check_url: string;
	proxy_check_list: string;
	proxy_check_type: string;
	custom_input_values: Record<string, string>;
}
