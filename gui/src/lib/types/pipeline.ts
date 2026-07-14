// TypeScript types mirroring Rust pipeline types

import type { BlockSettings } from './block-settings';

export interface Pipeline {
	id: string;
	name: string;
	author: string;
	created: string;
	modified: string;
	blocks: Block[];
	startup_blocks: Block[];
	data_settings: DataSettings;
	proxy_settings: ProxySettings;
	browser_settings: BrowserSettings;
	runner_settings: RunnerSettings;
	output_settings: OutputSettings;
}

export interface Block {
	id: string;
	block_type: BlockType;
	label: string;
	disabled: boolean;
	safe_mode: boolean;
	settings: BlockSettings;
}

export type BlockType =
	| 'HttpRequest' | 'ParseLR' | 'ParseRegex' | 'ParseJSON' | 'ParseCSS' | 'ParseXPath' | 'ParseCookie' | 'Parse'
	| 'KeyCheck' | 'StringFunction' | 'ListFunction' | 'CryptoFunction' | 'ConversionFunction'
	| 'IfElse' | 'Loop' | 'Delay' | 'Script' | 'Log' | 'SetVariable' | 'ClearCookies' | 'Webhook' | 'WebSocket'
	| 'TcpRequest' | 'UdpRequest' | 'FtpRequest' | 'SshRequest' | 'ImapRequest' | 'SmtpRequest' | 'PopRequest'
	| 'DateFunction' | 'CaseSwitch' | 'CookieContainer'
	| 'CaptchaSolver' | 'CloudflareBypass' | 'LaravelCsrf' | 'JwtToken' | 'HeaderSpoof'
	| 'BrowserOpen' | 'NavigateTo' | 'ClickElement' | 'TypeText' | 'WaitForElement' | 'GetElementText' | 'Screenshot' | 'ExecuteJs'
	| 'RandomUserAgent' | 'OcrCaptcha' | 'RecaptchaInvisible' | 'XacfSensor'
	| 'RandomData' | 'DataDomeSensor' | 'Plugin' | 'AkamaiV3Sensor' | 'Group'
	| 'ByteArray' | 'Constants' | 'Dictionary' | 'FloatFunction' | 'IntegerFunction' | 'TimeFunction' | 'GenerateGUID' | 'PhoneCountry' | 'LambdaParser'
	| 'FileSystem';

export type BotStatus = 'None' | 'Success' | 'Fail' | 'Ban' | 'Retry' | 'Error' | 'Custom' | 'ToCheck';
export type Comparison = 'Contains' | 'NotContains' | 'EqualTo' | 'NotEqualTo' | 'MatchesRegex' | 'GreaterThan' | 'LessThan' | 'Exists' | 'NotExists';

export interface DataSettings {
	wordlist_type: string;
	separator: string;
	slices: string[];
}

export interface ProxySettings {
	proxy_mode: 'None' | 'Rotate' | 'Sticky' | 'CpmLimited';
	proxy_sources: ProxySource[];
	ban_duration_secs: number;
	max_retries_before_ban: number;
	cpm_per_proxy: number;
	proxy_groups: ProxyGroup[];
	active_group: string;
}

export interface ProxyGroup {
	name: string;
	mode: 'None' | 'Rotate' | 'Sticky' | 'CpmLimited';
	sources: ProxySource[];
	cpm_per_proxy: number;
}

export type ProxySourceType_t = 'Http' | 'Https' | 'Socks4' | 'Socks5' | 'Shadowsocks';

export interface ProxySource {
	source_type: 'File' | 'Url' | 'Inline';
	value: string;
	refresh_interval_secs: number;
	/** Protocol to apply to plain host:port lines that carry no prefix. */
	default_proxy_type?: ProxySourceType_t;
}

export interface BrowserSettings {
	browser: string;
	ja3: string | null;
	http2_fingerprint: string | null;
	user_agent: string | null;
}

export interface RunnerSettings {
	threads: number;
	skip: number;
	take: number;
	continue_statuses: BotStatus[];
	custom_status_name: string;
	max_retries: number;
	concurrent_per_proxy: number;
	start_threads_gradually: boolean;
	gradual_delay_ms: number;
	automatic_thread_count: boolean;
	lower_threads_on_retry: boolean;
	retry_thread_reduction_pct: number;
	pause_on_ratelimit: boolean;
	only_proxyless: boolean;
}

export interface OutputSettings {
	save_to_file: boolean;
	save_to_database: boolean;
	include_response: boolean;
	output_directory: string;
	output_format: string;
	database_path: string;
	output_format_type: 'Txt' | 'Csv' | 'Json';
	capture_filters: CaptureFilter[];
}

export interface CaptureFilter {
	variable_name: string;
	filter_type: CaptureFilterType;
	value: string;
	negate: boolean;
}

export type CaptureFilterType = 'Contains' | 'Equals' | 'StartsWith' | 'EndsWith' | 'MatchesRegex' | 'MinLength' | 'MaxLength' | 'NotEmpty';
