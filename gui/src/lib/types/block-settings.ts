import type { Block, BotStatus, Comparison } from './pipeline';

export type BlockSettings =
	| { type: 'HttpRequest' } & HttpRequestSettings
	| { type: 'ParseLR' } & ParseLRSettings
	| { type: 'ParseRegex' } & ParseRegexSettings
	| { type: 'ParseJSON' } & ParseJSONSettings
	| { type: 'ParseCSS' } & ParseCSSSettings
	| { type: 'ParseXPath' } & ParseXPathSettings
	| { type: 'ParseCookie' } & ParseCookieSettings
	| { type: 'Parse' } & ParseSettings
	| { type: 'KeyCheck' } & KeyCheckSettings
	| { type: 'StringFunction' } & StringFunctionSettings
	| { type: 'ListFunction' } & ListFunctionSettings
	| { type: 'CryptoFunction' } & CryptoFunctionSettings
	| { type: 'ConversionFunction' } & ConversionFunctionSettings
	| { type: 'IfElse' } & IfElseSettings
	| { type: 'Loop' } & LoopSettings
	| { type: 'Delay' } & DelaySettings
	| { type: 'Script' } & ScriptSettings
	| { type: 'Log' } & LogSettings
	| { type: 'SetVariable' } & SetVariableSettings
	| { type: 'ClearCookies' }
	| { type: 'Webhook' } & WebhookSettings
	| { type: 'WebSocket' } & WebSocketSettings
	| { type: 'TcpRequest' } & TcpRequestSettings
	| { type: 'UdpRequest' } & UdpRequestSettings
	| { type: 'FtpRequest' } & FtpRequestSettings
	| { type: 'SshRequest' } & SshRequestSettings
	| { type: 'ImapRequest' } & ImapRequestSettings
	| { type: 'SmtpRequest' } & SmtpRequestSettings
	| { type: 'PopRequest' } & PopRequestSettings
	| { type: 'DateFunction' } & DateFunctionSettings
	| { type: 'CaseSwitch' } & CaseSwitchSettings
	| { type: 'CookieContainer' } & CookieContainerSettings
	| { type: 'CaptchaSolver' } & CaptchaSolverSettings
	| { type: 'CloudflareBypass' } & CloudflareBypassSettings
	| { type: 'LaravelCsrf' } & LaravelCsrfSettings
	| { type: 'BrowserOpen' } & BrowserOpenSettings
	| { type: 'NavigateTo' } & NavigateToSettings
	| { type: 'ClickElement' } & ClickElementSettings
	| { type: 'TypeText' } & TypeTextSettings
	| { type: 'WaitForElement' } & WaitForElementSettings
	| { type: 'GetElementText' } & GetElementTextSettings
	| { type: 'Screenshot' } & ScreenshotSettings
	| { type: 'ExecuteJs' } & ExecuteJsSettings
	| { type: 'RandomUserAgent' } & RandomUserAgentSettings
	| { type: 'OcrCaptcha' } & OcrCaptchaSettings
	| { type: 'RecaptchaInvisible' } & RecaptchaInvisibleSettings
	| { type: 'XacfSensor' } & XacfSensorSettings
	| { type: 'JwtToken' } & JwtTokenSettings
	| { type: 'HeaderSpoof' } & HeaderSpoofSettings
	| { type: 'RandomData' } & RandomDataSettings
	| { type: 'DataDomeSensor' } & DataDomeSensorSettings
	| { type: 'Plugin' } & PluginBlockSettings
	| { type: 'AkamaiV3Sensor' } & AkamaiV3SensorSettings
	| { type: 'Group' } & GroupSettings
	| { type: 'ByteArray' } & ByteArraySettings
	| { type: 'Constants' } & ConstantsSettings
	| { type: 'Dictionary' } & DictionarySettings
	| { type: 'FloatFunction' } & FloatFunctionSettings
	| { type: 'IntegerFunction' } & IntegerFunctionSettings
	| { type: 'TimeFunction' } & TimeFunctionSettings
	| { type: 'GenerateGUID' } & GenerateGUIDSettings
	| { type: 'PhoneCountry' } & PhoneCountrySettings
	| { type: 'LambdaParser' } & LambdaParserSettings

	| { type: 'FileSystem' } & FileSystemSettings;

export interface HttpRequestSettings {
	method: string;
	url: string;
	headers: [string, string][];
	body: string;
	body_type: 'None' | 'Standard' | 'Raw' | 'Multipart' | 'BasicAuth';
	content_type: string;
	follow_redirects: boolean;
	max_redirects: number;
	timeout_ms: number;
	auto_redirect: boolean;
	basic_auth: [string, string] | null;
	http_version: string;
	response_var: string;
	custom_cookies: string;
	/** When false, skip TLS certificate verification (for debugging / self-signed certs) */
	ssl_verify: boolean;
	proxy_insecure: boolean;
	/** Dash-separated IANA cipher suite IDs — overrides browser profile defaults. Empty = use profile default. */
	cipher_suites: string;
	/**
	 * TLS client to use for this request block.
	 * - 'RustTLS' (default): Rust-native reqwest + rustls — no fingerprinting, standard HTTPS.
	 * - 'AzureTLS': Go sidecar with azuretls — JA3/browser fingerprinting, custom ciphers.
	 * - 'WreqTLS': Rust-native wreq + BoringSSL — 100+ browser emulation profiles, cookie persistence.
	 */
	tls_client: 'AzureTLS' | 'RustTLS' | 'WreqTLS';
	/** AzureTLS browser profile override. Empty = inherit pipeline browser settings. */
	browser_profile: string;
	/** Per-block AzureTLS JA3 override. Empty = inherit pipeline settings. */
	ja3_override: string;
	/** Per-block AzureTLS HTTP/2 fingerprint override. Empty = inherit pipeline settings. */
	http2fp_override: string;
	/** WreqTLS emulation profile label. e.g. "Chrome134", "Firefox136", "Safari18". Default = "Chrome134". */
	wreq_emulation: string;
}

export interface ParseLRSettings {
	input_var: string;
	left: string;
	right: string;
	output_var: string;
	capture: boolean;
	recursive: boolean;
	case_insensitive: boolean;
}

export interface ParseRegexSettings {
	input_var: string;
	pattern: string;
	output_format: string;
	output_var: string;
	capture: boolean;
	multi_line: boolean;
}

export interface ParseJSONSettings {
	input_var: string;
	json_path: string;
	output_var: string;
	capture: boolean;
}

export interface ParseCSSSettings {
	input_var: string;
	selector: string;
	attribute: string;
	output_var: string;
	capture: boolean;
	index: number;
}

export interface ParseXPathSettings {
	input_var: string;
	xpath: string;
	output_var: string;
	capture: boolean;
}

export interface ParseCookieSettings {
	input_var: string;
	cookie_name: string;
	output_var: string;
	capture: boolean;
}

export type ParseMode = 'LR' | 'Regex' | 'Json' | 'Css' | 'XPath' | 'Cookie' | 'Lambda';

/** Unified parse block — replaces ParseLR/ParseRegex/ParseJSON/ParseCSS/ParseXPath/ParseCookie/LambdaParser */
export interface ParseSettings {
	parse_mode: ParseMode;
	// Common
	input_var: string;
	output_var: string;
	capture: boolean;
	// LR
	left: string;
	right: string;
	recursive: boolean;
	case_insensitive: boolean;
	// Regex
	pattern: string;
	output_format: string;
	multi_line: boolean;
	// JSON
	json_path: string;
	// CSS
	selector: string;
	attribute: string;
	index: number;
	// XPath
	xpath: string;
	// Cookie
	cookie_name: string;
	// Lambda
	lambda_expression: string;
}

export interface KeyCheckSettings {
	keychains: Keychain[];
	/** When true, a Fail result halts the pipeline immediately — skips all
	 *  subsequent Parse/function blocks for this data entry. */
	stop_on_fail: boolean;
}

export interface Keychain {
	result: BotStatus;
	conditions: KeyCondition[];
	/** How conditions are joined. 'And' = all must match; 'Or' = any one is enough. */
	mode: 'And' | 'Or';
}

export interface KeyCondition {
	source: string;
	comparison: Comparison;
	value: string;
}

export interface StringFunctionSettings {
	function_type: string;
	input_var: string;
	output_var: string;
	capture: boolean;
	param1: string;
	param2: string;
}

export interface ListFunctionSettings {
	function_type: string;
	input_var: string;
	output_var: string;
	capture: boolean;
	param1: string;
}

export interface CryptoFunctionSettings {
	function_type: string;
	input_var: string;
	output_var: string;
	capture: boolean;
	key: string;
}

export interface ConversionFunctionSettings {
	op: string;
	input_var: string;
	output_var: string;
	capture: boolean;
	encoding?: string;
	endianness?: string;
	byte_count?: number;
}

export interface DateFunctionSettings {
	function_type: string;
	input_var: string;
	output_var: string;
	format: string;
	amount: number;
	unit: string;
	capture: boolean;
	/** Expression for Compute, decimal places for Round */
	param: string;
}

export interface CaseSwitchSettings {
	input_var: string;
	cases: CaseBranch[];
	default_value: string;
	output_var: string;
	capture: boolean;
}

export interface CaseBranch {
	match_value: string;
	result_value: string;
}

export interface CookieContainerSettings {
	source: string;
	source_type: string;
	domain: string;
	output_var: string;
	capture: boolean;
	save_netscape: boolean;
}

export interface WebhookSettings {
	url: string;
	method: string;
	headers: [string, string][];
	body_template: string;
	content_type: string;
	custom_cookies: string;
}

export interface WebSocketSettings {
	url: string;
	action: string;
	message: string;
	output_var: string;
	timeout_ms: number;
}

export interface TcpRequestSettings {
	host: string;
	port: number;
	data: string;
	output_var: string;
	timeout_ms: number;
	use_tls: boolean;
	ssl_verify: boolean;
	capture: boolean;
}

export interface UdpRequestSettings {
	host: string;
	port: number;
	data: string;
	output_var: string;
	timeout_ms: number;
	capture: boolean;
}

export interface FtpRequestSettings {
	host: string;
	port: number;
	username: string;
	password: string;
	command: string;
	/** Remote path for RETR, STOR, DELE, MKD, RMD, CWD actions */
	remote_path: string;
	/** Local file to upload (STOR) */
	local_path: string;
	/** Local directory to save downloaded file (RETR) */
	output_dir: string;
	output_var: string;
	timeout_ms: number;
	capture: boolean;
}

export interface SshRequestSettings {
	host: string;
	port: number;
	username: string;
	password: string;
	command: string;
	output_var: string;
	timeout_ms: number;
	capture: boolean;
}

export interface ImapRequestSettings {
	host: string;
	port: number;
	username: string;
	password: string;
	use_tls: boolean;
	ssl_verify: boolean;
	command: string;
	/** Mailbox for SELECT / FETCH / SEARCH actions */
	mailbox: string;
	/** Message number for FETCH action */
	message_num: number;
	output_var: string;
	timeout_ms: number;
	capture: boolean;
}

export interface SmtpRequestSettings {
	host: string;
	port: number;
	username: string;
	password: string;
	use_tls: boolean;
	ssl_verify: boolean;
	command: string;
	/** "VERIFY" or "SEND_EMAIL" */
	action: string;
	from: string;
	to: string;
	subject: string;
	body_template: string;
	output_var: string;
	timeout_ms: number;
	capture: boolean;
}

export interface PopRequestSettings {
	host: string;
	port: number;
	username: string;
	password: string;
	use_tls: boolean;
	ssl_verify: boolean;
	command: string;
	/** Message number for RETR / DELE actions */
	message_num: number;
	output_var: string;
	timeout_ms: number;
	capture: boolean;
}

export interface CaptchaSolverSettings {
	solver_service: string;
	api_key: string;
	site_key: string;
	page_url: string;
	captcha_type: string;
	output_var: string;
	timeout_ms: number;
	capture: boolean;
}

export interface CloudflareBypassSettings {
	url: string;
	flaresolverr_url: string;
	max_timeout_ms: number;
	output_var: string;
	capture: boolean;
}

export interface LaravelCsrfSettings {
	url: string;
	csrf_selector: string;
	cookie_name: string;
	output_var: string;
	timeout_ms: number;
	capture: boolean;
}

export interface RandomUserAgentSettings {
	mode: 'Random' | 'CustomList';
	browser_filter: string[];
	platform_filter: string[];
	custom_list: string;
	output_var: string;
	capture: boolean;
	match_tls: boolean;
}

export interface OcrCaptchaSettings {
	input_var: string;
	language: string;
	psm: number;
	whitelist: string;
	output_var: string;
	capture: boolean;
}

export interface RecaptchaInvisibleSettings {
	ar: string;
	sitekey: string;
	co: string;
	hi: string;
	v: string;
	size: string;
	action: string;
	cb: string;
	anchor_url: string;
	reload_url: string;
	user_agent: string;
	output_var: string;
	capture: boolean;
}

export interface XacfSensorSettings {
	bundle_id: string;
	version: string;
	output_var: string;
	capture: boolean;
}

export type RandomDataType = 'String' | 'Uuid' | 'Number' | 'Email' | 'FirstName' | 'LastName' | 'FullName' | 'StreetAddress' | 'City' | 'State' | 'ZipCode' | 'PhoneNumber' | 'Date';

export interface RandomDataSettings {
	data_type: RandomDataType;
	output_var: string;
	capture: boolean;
	string_length: number;
	string_charset: string;
	custom_chars: string;
	number_min: number;
	number_max: number;
	number_decimal: boolean;
	date_format: string;
	date_min: string;
	date_max: string;
}

export interface DataDomeSensorSettings {
	site_url: string;
	cookie_datadome: string;
	user_agent: string;
	custom_wasm_b64: string;
	output_var: string;
	capture: boolean;
}

export type AkamaiV3Mode = 'Encrypt' | 'Decrypt' | 'ExtractCookieHash';

export interface AkamaiV3SensorSettings {
	mode: AkamaiV3Mode;
	payload_var: string;
	file_hash: string;
	cookie_hash: string;
	output_var: string;
	capture: boolean;
}

export interface PluginBlockSettings {
	plugin_block_type: string;
	settings_json: string;
	output_var: string;
	capture: boolean;
}

export interface GroupSettings {
	blocks: Block[];
	collapsed: boolean;
}

export interface BrowserOpenSettings {
	headless: boolean;
	browser_type: string;
	proxy: string;
	extra_args: string;
}

export interface NavigateToSettings {
	url: string;
	wait_until: string;
	timeout_ms: number;
	custom_cookies: string;
}

export interface ClickElementSettings {
	selector: string;
	wait_for_navigation: boolean;
	timeout_ms: number;
}

export interface TypeTextSettings {
	selector: string;
	text: string;
	clear_first: boolean;
	delay_ms: number;
}

export interface WaitForElementSettings {
	selector: string;
	state: string;
	timeout_ms: number;
}

export interface GetElementTextSettings {
	selector: string;
	attribute: string;
	output_var: string;
	capture: boolean;
}

export interface ScreenshotSettings {
	full_page: boolean;
	selector: string;
	output_var: string;
}

export interface ExecuteJsSettings {
	code: string;
	output_var: string;
	capture: boolean;
}

export interface IfElseSettings {
	condition: KeyCondition;
	true_blocks: Block[];
	false_blocks: Block[];
}

export interface LoopSettings {
	loop_type: 'ForEach' | 'Repeat';
	list_var: string;
	item_var: string;
	count: number;
	blocks: Block[];
}

export interface DelaySettings {
	min_ms: number;
	max_ms: number;
}

export interface ScriptSettings {
	code: string;
	output_var: string;
	capture: boolean;
}

export interface LogSettings {
	message: string;
}

export interface SetVariableSettings {
	name: string;
	value: string;
	capture: boolean;
}

export interface ByteArraySettings {
	operation: 'ToHex' | 'FromHex' | 'ToBase64' | 'FromBase64' | 'ToUtf8' | 'FromUtf8';
	input_var: string;
	output_var: string;
	encoding: string;
	capture: boolean;
}

export interface ConstantsSettings {
	constants: { name: string; value: string }[];
}

export interface DictionarySettings {
	operation: 'Get' | 'Set' | 'Remove' | 'Exists' | 'Keys' | 'Values';
	dict_var: string;
	key: string;
	value: string;
	output_var: string;
	capture: boolean;
}

export interface FloatFunctionSettings {
	function_type: 'Round' | 'Ceil' | 'Floor' | 'Abs' | 'Add' | 'Subtract' | 'Multiply' | 'Divide' | 'Power' | 'Sqrt' | 'Min' | 'Max';
	input_var: string;
	param1: string;
	param2: string;
	output_var: string;
	capture: boolean;
}

export interface IntegerFunctionSettings {
	function_type: 'Add' | 'Subtract' | 'Multiply' | 'Divide' | 'Modulo' | 'Power' | 'Abs' | 'Min' | 'Max';
	input_var: string;
	param1: string;
	param2: string;
	output_var: string;
	capture: boolean;
}

export interface TimeFunctionSettings {
	function_type: 'ConvertTimezone' | 'GetTimezone' | 'IsDST' | 'DurationBetween' | 'AddDuration' | 'SubtractDuration';
	input_var: string;
	timezone: string;
	target_timezone: string;
	format: string;
	output_var: string;
	capture: boolean;
}

export interface GenerateGUIDSettings {
	guid_version: 'V1' | 'V4' | 'V5';
	namespace: string;
	name: string;
	output_var: string;
	capture: boolean;
}

export interface PhoneCountrySettings {
	input_var: string;
	output_var: string;
	output_format: 'CountryCode' | 'CountryName' | 'ISO2' | 'ISO3';
	capture: boolean;
}

export interface LambdaParserSettings {
	input_var: string;
	lambda_expression: string;
	output_var: string;
	capture: boolean;
}

export interface FileSystemSettings {
	op: 'CreatePath' | 'FileAppend' | 'FileAppendLines' | 'FileCopy' | 'FileMove'
		| 'FileDelete' | 'FileExists' | 'FileRead' | 'FileReadBytes' | 'FileReadLines'
		| 'FileWrite' | 'FileWriteBytes' | 'FileWriteLines' | 'FolderDelete'
		| 'FolderExists' | 'GetFilesInFolder';
	path: string;
	dest_path: string;
	content: string;
	encoding: string;
	output_var: string;
	capture: boolean;
}

export interface JwtTokenSettings {
	action: 'Sign' | 'Decode';
	secret: string;
	algorithm: 'HS256' | 'HS384' | 'HS512';
	claims: string;
	token_input: string;
	output_var: string;
	expires_in_secs: number;
	verify_on_decode: boolean;
}

export interface HeaderSpoofSettings {
	inject_xff: boolean;
	inject_x_real_ip: boolean;
	inject_cf_connecting_ip: boolean;
	inject_true_client_ip: boolean;
	strategy: 'RandomPublic' | 'FixedList' | 'FromProxy' | 'Manual';
	fixed_ips: string;
	manual_value: string;
	output_var: string;
	inject_proto: boolean;
	inject_host: boolean;
}
