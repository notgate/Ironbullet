import type { Block } from '$lib/types';

// --- Variable embed detection ---
export const varPattern = /<[a-zA-Z_][a-zA-Z0-9_.]*>/;
export function hasVars(val: string | undefined): boolean {
	return !!val && varPattern.test(val);
}

// --- Shared input class strings ---
export const inputCls = "w-full skeu-input font-mono mt-0.5";
export const labelCls = "text-[10px] uppercase tracking-wider text-muted-foreground";
export const hintCls = "text-[9px] text-muted-foreground/60 mt-0.5";
export const smallInputCls = "skeu-input text-[10px] font-mono";

// --- Block documentation / field hints ---
export const BLOCK_DOCS: Record<string, { summary: string; fields: Record<string, string> }> = {
	HttpRequest: {
		summary: 'Sends an HTTP request to a target URL and stores the response.',
		fields: {
			url: 'Target URL. Supports <VAR> interpolation.',
			method: 'HTTP method for the request.',
			headers: 'Custom HTTP headers sent with the request.',
			body: 'Request body content (for POST, PUT, PATCH).',
			follow_redirects: 'Automatically follow 3xx redirects.',
			timeout_ms: 'Max time to wait for a response in milliseconds.',
			http_version: 'HTTP protocol version to use.',
			response_var: 'Variable name to store the response body.',
		}
	},
	ParseLR: {
		summary: 'Extracts text between left and right delimiters from a string.',
		fields: {
			input_var: 'Source variable containing the text to parse.',
			left: 'String that appears before the target text.',
			right: 'String that appears after the target text.',
			output_var: 'Variable name to store the extracted value.',
			recursive: 'Find all matches instead of just the first.',
			capture: 'Save the result to the hit output.',
		}
	},
	ParseJSON: {
		summary: 'Extracts a value from JSON using a dot-notation path.',
		fields: {
			input_var: 'Variable containing JSON string.',
			json_path: 'Dot-notation path (e.g. user.token, items[0].id).',
			output_var: 'Variable name to store the extracted value.',
		}
	},
	ParseRegex: {
		summary: 'Extracts text using a regular expression pattern.',
		fields: {
			input_var: 'Variable containing the text to search.',
			pattern: 'Regular expression pattern with capture groups.',
			output_format: 'Format string using $1, $2, etc. for groups.',
			output_var: 'Variable to store the result.',
		}
	},
	ParseCSS: {
		summary: 'Extracts text or attributes from HTML using CSS selectors.',
		fields: {
			selector: 'CSS selector (e.g. div.class > a, #id).',
			attribute: 'HTML attribute to extract. Empty = text content.',
			index: 'Element index (0-based). -1 extracts all matches.',
		}
	},
	ParseXPath: {
		summary: 'Extracts data from HTML/XML using XPath expressions.',
		fields: { xpath: 'XPath expression (e.g. //div[@class="result"]/text()).' }
	},
	ParseCookie: {
		summary: 'Extracts a specific cookie value from the response cookies.',
		fields: { cookie_name: 'Name of the cookie to extract.' }
	},
	KeyCheck: {
		summary: 'Checks conditions against variables to set the bot status (Success, Fail, Ban, etc.).',
		fields: {
			keychains: 'Each keychain defines conditions that, when ALL match, set a result status.',
			source: 'Variable to check (e.g. data.SOURCE, data.RESPONSECODE).',
			comparison: 'How to compare the source against the value.',
			value: 'Expected value to compare against.',
		}
	},
	StringFunction: {
		summary: 'Performs a string operation (replace, trim, encode, etc.) on a variable.',
		fields: {
			function_type: 'String operation to perform.',
			input_var: 'Variable containing the input string.',
		}
	},
	Translate: {
		summary: 'Maps an input value through a deterministic local lookup table.',
		fields: { mappings: 'One source => destination exact mapping per line. Unmatched values pass through unchanged.' }
	},
	ListFunction: {
		summary: 'Performs a list operation (join, sort, add, remove, etc.).',
		fields: { input_var: 'Variable containing the list (prefix with @).' }
	},
	CryptoFunction: {
		summary: 'Hashes or encrypts data using the selected algorithm.',
		fields: { key: 'Secret key required for HMAC and AES operations.' }
	},
	ConversionFunction: {
		summary: 'Converts data between types and encodings: type casts, Base64, Hex, URL, HTML, Bytes, BigInt, Binary, readable sizes, number words, and SVG→PNG.',
		fields: {
			op: 'Conversion operation. Type casts: StringToInt, IntToFloat, etc. Encoding: Base64Encode/Decode, HexEncode/Decode, UrlEncode/Decode, HtmlEncode/Decode. Bytes: StringToBytes, BytesToString, IntToBytes, BytesToInt, BigIntToBytes, BytesToBigInt, BytesToBinaryString, BinaryStringToBytes. Formatting: ReadableSize, NumberToWords, WordsToNumber. Rendering: SvgToPng.',
			input_var: 'Variable containing the input value.',
			output_var: 'Variable to store the converted result.',
			encoding: 'String encoding for StringToBytes / BytesToString (utf8, utf16).',
			endianness: 'Byte order for IntToBytes / BigIntToBytes (big or little).',
			byte_count: 'Number of output bytes for IntToBytes (1–8).',
		}
	},
	IfElse: {
		summary: 'Branches execution based on a condition. Blocks in the True branch run when the condition matches.',
		fields: { condition: 'Source variable, comparison operator, and expected value.' }
	},
	Loop: {
		summary: 'Repeats nested blocks for each item in a list or a fixed number of times.',
		fields: {
			list_var: 'Variable containing the list to iterate (prefix with @).',
			item_var: 'Variable name for the current item in each iteration.',
			count: 'Number of times to repeat the loop body.',
		}
	},
	Delay: {
		summary: 'Pauses execution for a random duration between min and max milliseconds.',
		fields: {}
	},
	Script: {
		summary: 'Executes custom JavaScript code with access to pipeline variables.',
		fields: { code: 'JavaScript code to execute. Use return to output a value.' }
	},
	Log: {
		summary: 'Writes a message to the debug log. Useful for inspecting variable values.',
		fields: { message: 'Log text. Use <VAR> for variable values.' }
	},
	SetVariable: {
		summary: 'Creates or updates a variable with a specific value.',
		fields: { name: 'Variable name to set.', value: 'Value to assign.' }
	},
	ClearCookies: {
		summary: 'Clears all cookies stored in the current session. No parameters needed.',
		fields: {}
	},
	Webhook: {
		summary: 'Sends data to an external webhook URL (e.g. Discord, Slack).',
		fields: {
			url: 'Webhook endpoint URL.',
			body_template: 'JSON body template. Use <VAR> for variable values.',
		}
	},
	WebSocket: {
		summary: 'Manages WebSocket connections: connect, send, receive, or close.',
		fields: { action: 'WebSocket operation to perform.' }
	},
	TcpRequest: {
		summary: 'Sends raw data over a TCP connection and reads the response.',
		fields: { use_tls: 'Encrypt the connection with TLS/SSL.' }
	},
	UdpRequest: {
		summary: 'Sends a UDP datagram and optionally reads a response.',
		fields: {}
	},
	FtpRequest: { summary: 'Connects to an FTP server and executes a command.', fields: {} },
	SshRequest: { summary: 'Connects to an SSH server and executes a remote command.', fields: {} },
	ImapRequest: { summary: 'Connects to an IMAP mail server to check or fetch email.', fields: {} },
	SmtpRequest: { summary: 'Connects to an SMTP server to send email.', fields: {} },
	PopRequest: { summary: 'Connects to a POP3 mail server to retrieve email.', fields: {} },
	CaptchaSolver: {
		summary: 'Solves a captcha using a third-party solver service API.',
		fields: {
			solver_service: 'Captcha solving service provider.',
			captcha_type: 'Type of captcha on the target page.',
			api_key: 'Your API key for the solver service.',
			site_key: 'The captcha site key from the target page HTML.',
			page_url: 'URL of the page containing the captcha.',
		}
	},
	CloudflareBypass: {
		summary: 'Bypasses Cloudflare protection using a FlareSolverr instance.',
		fields: {
			url: 'URL of the Cloudflare-protected page.',
			flaresolverr_url: 'Local FlareSolverr API endpoint.',
		}
	},
	LaravelCsrf: {
		summary: 'Fetches a Laravel CSRF token from a page for form submissions.',
		fields: {
			csrf_selector: 'CSS selector for the hidden CSRF input field.',
			cookie_name: 'Name of the CSRF cookie.',
		}
	},
	BrowserOpen: {
		summary: 'Opens a headless or visible browser instance for automation.',
		fields: {
			browser_type: 'Browser engine to use.',
			headless: 'Run without visible browser window.',
			extra_args: 'Additional command-line flags for the browser.',
		}
	},
	NavigateTo: {
		summary: 'Navigates the browser to a URL and waits for the page to load.',
		fields: { wait_until: 'When to consider the page fully loaded.' }
	},
	ClickElement: {
		summary: 'Clicks an element on the page matching the CSS selector.',
		fields: { wait_for_navigation: 'Wait for a page navigation after clicking.' }
	},
	TypeText: {
		summary: 'Types text into an input field with optional keystroke delay.',
		fields: {
			clear_first: 'Clear the field before typing new text.',
			delay_ms: 'Delay between keystrokes in milliseconds.',
		}
	},
	WaitForElement: {
		summary: 'Waits for an element to appear, disappear, or change state.',
		fields: { state: 'Element state to wait for.' }
	},
	GetElementText: {
		summary: 'Reads text content or an attribute from a page element.',
		fields: { attribute: 'HTML attribute to read. Empty = inner text.' }
	},
	GetDom: {
		summary: 'Reads the full current page DOM or a selected element HTML.',
		fields: { selector: 'Optional CSS selector. Empty reads the complete page DOM.', outer_html: 'When selected, include the matching element itself rather than only its children.' }
	},
	Screenshot: {
		summary: 'Takes a screenshot of the page or a specific element.',
		fields: { full_page: 'Capture the entire scrollable page.' }
	},
	ExecuteJs: {
		summary: 'Executes JavaScript code in the browser page context.',
		fields: { code: 'JS code to run. Use return to capture a value.' }
	},
	DateFunction: {
		summary: 'Performs date/time operations: get current time, format, parse, or add/subtract time.',
		fields: {
			function_type: 'Date operation to perform.',
			input_var: 'Variable containing the date string to process.',
			format: 'Date format string using strftime syntax (e.g. %Y-%m-%d %H:%M:%S).',
			amount: 'Number of time units to add or subtract.',
			unit: 'Time unit for add/subtract operations.',
		}
	},
	CaseSwitch: {
		summary: 'Maps an input value to a result using case matching. Like a switch/case statement.',
		fields: {
			input_var: 'Variable to match against each case value.',
			cases: 'List of value -> result mappings checked in order.',
			default_value: 'Result when no case matches the input.',
			output_var: 'Variable to store the matched result.',
		}
	},
	CookieContainer: {
		summary: 'Reads cookies from a file (Netscape format) or raw text, optionally filters by domain, and stores them in a variable. Based on OpenBullet Cookie Edition.',
		fields: {
			source: 'File path (if source_type=file) or raw cookie text.',
			source_type: 'Whether to read from a "file" or use "text" directly.',
			domain: 'Domain filter -- only include cookies matching this domain.',
			output_var: 'Variable to store cookies as name=value; pairs.',
			save_netscape: 'Also store in Netscape format as {output_var}_NETSCAPE.',
		}
	},
	Group: {
		summary: 'Organizational container that groups blocks together. Child blocks execute sequentially. Toggle collapsed to show/hide contents.',
		fields: {
			collapsed: 'Whether the group is visually collapsed in the editor.',
		}
	},

	FileSystem: {
		summary: 'Performs file and folder operations: read, write, append, copy, move, delete, exists checks, and directory listing.',
		fields: {
			op: 'File system operation to perform.',
			path: 'File or folder path. Supports <VAR> interpolation.',
			dest_path: 'Destination path for Copy and Move operations.',
			content: 'Content to write or append. Supports <VAR> interpolation.',
			output_var: 'Variable to store the result (for read/exists/list operations).',
		}
	},
};

export function fieldHint(block: Block, field: string): string {
	const docs = BLOCK_DOCS[block.settings.type];
	return docs?.fields[field] || '';
}

// --- Shared option arrays ---
export const COMPARISON_OPTIONS = [
	{value:'Contains',label:'Contains'},{value:'NotContains',label:'Not Contains'},
	{value:'EqualTo',label:'Equals'},{value:'NotEqualTo',label:'Not Equals'},
	{value:'MatchesRegex',label:'Regex'},{value:'GreaterThan',label:'Greater'},
	{value:'LessThan',label:'Less'},{value:'Exists',label:'Exists'},{value:'NotExists',label:'Not Exists'},
];

export const CONVERSION_TYPE_OPTIONS = [
	{value:'String',label:'String'},{value:'Int',label:'Integer'},{value:'Float',label:'Float'},
	{value:'Bool',label:'Boolean'},{value:'Hex',label:'Hex'},{value:'Base64',label:'Base64'},
];

export const HTTP_VERSION_OPTIONS = [
	{ value: 'HTTP/1.1', label: 'HTTP/1.1' },
	{ value: 'HTTP/2', label: 'HTTP/2' },
	{ value: 'HTTP/3', label: 'HTTP/3' },
];

// --- String Function param labels ---
export const STRING_FUNCTIONS = [
	{ value: 'Replace', label: 'Replace', p1: 'Find', p2: 'Replace with' },
	{ value: 'Substring', label: 'Substring', p1: 'Start index', p2: 'Length' },
	{ value: 'Trim', label: 'Trim', p1: '', p2: '' },
	{ value: 'ToUpper', label: 'To Upper', p1: '', p2: '' },
	{ value: 'ToLower', label: 'To Lower', p1: '', p2: '' },
	{ value: 'URLEncode', label: 'URL Encode', p1: '', p2: '' },
	{ value: 'URLDecode', label: 'URL Decode', p1: '', p2: '' },
	{ value: 'Base64Encode', label: 'Base64 Encode', p1: '', p2: '' },
	{ value: 'Base64Decode', label: 'Base64 Decode', p1: '', p2: '' },
	{ value: 'HTMLEncode', label: 'HTML Encode', p1: '', p2: '' },
	{ value: 'HTMLDecode', label: 'HTML Decode', p1: '', p2: '' },
	{ value: 'Split', label: 'Split', p1: 'Separator', p2: '' },
	{ value: 'RandomString', label: 'Random String', p1: 'Length', p2: 'Charset (abc123...)' },
];

export function getStringFuncMeta(ft: string) {
	return STRING_FUNCTIONS.find(f => f.value === ft) || STRING_FUNCTIONS[0];
}

// --- List Function options ---
export const LIST_FUNCTIONS = [
	{ value: 'Join', label: 'Join', param: 'Separator' },
	{ value: 'Sort', label: 'Sort', param: '' },
	{ value: 'Shuffle', label: 'Shuffle', param: '' },
	{ value: 'Add', label: 'Add Item', param: 'Item' },
	{ value: 'Remove', label: 'Remove Item', param: 'Item' },
	{ value: 'Deduplicate', label: 'Deduplicate', param: '' },
	{ value: 'RandomItem', label: 'Random Item', param: '' },
	{ value: 'Length', label: 'Length', param: '' },
];

export function getListFuncMeta(ft: string) {
	return LIST_FUNCTIONS.find(f => f.value === ft) || LIST_FUNCTIONS[0];
}

// --- Crypto Function options ---
export const CRYPTO_FUNCTIONS = [
	{ value: 'MD5', label: 'MD5', needsKey: false },
	{ value: 'SHA1', label: 'SHA-1', needsKey: false },
	{ value: 'SHA256', label: 'SHA-256', needsKey: false },
	{ value: 'SHA512', label: 'SHA-512', needsKey: false },
	{ value: 'SHA384', label: 'SHA-384', needsKey: false },
	{ value: 'CRC32', label: 'CRC32', needsKey: false },
	{ value: 'HMACSHA256', label: 'HMAC-SHA256', needsKey: true },
	{ value: 'HMACSHA512', label: 'HMAC-SHA512', needsKey: true },
	{ value: 'HMACMD5', label: 'HMAC-MD5', needsKey: true },
	{ value: 'BCryptHash', label: 'BCrypt Hash', needsKey: false },
	{ value: 'BCryptVerify', label: 'BCrypt Verify', needsKey: true },
	{ value: 'Base64Encode', label: 'Base64 Encode', needsKey: false },
	{ value: 'Base64Decode', label: 'Base64 Decode', needsKey: false },
	{ value: 'AESEncrypt', label: 'AES Encrypt', needsKey: true },
	{ value: 'AESDecrypt', label: 'AES Decrypt', needsKey: true },
];

export function getCryptoFuncMeta(ft: string) {
	return CRYPTO_FUNCTIONS.find(f => f.value === ft) || CRYPTO_FUNCTIONS[0];
}

export const CONVERSION_OPS = [
	// Type casts
	{ value: 'StringToInt',        label: 'String → Int',           category: 'Type Casts',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'IntToString',        label: 'Int → String',           category: 'Type Casts',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'StringToFloat',      label: 'String → Float',         category: 'Type Casts',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'FloatToString',      label: 'Float → String',         category: 'Type Casts',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'BoolToString',       label: 'Bool → String',          category: 'Type Casts',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'StringToBool',       label: 'String → Bool',          category: 'Type Casts',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'IntToFloat',         label: 'Int → Float',            category: 'Type Casts',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'FloatToInt',         label: 'Float → Int',            category: 'Type Casts',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	// Encoding
	{ value: 'Base64Encode',       label: 'Base64 Encode',          category: 'Encoding',    needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'Base64Decode',       label: 'Base64 Decode',          category: 'Encoding',    needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'HexEncode',          label: 'Hex Encode',             category: 'Encoding',    needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'HexDecode',          label: 'Hex Decode',             category: 'Encoding',    needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'UrlEncode',          label: 'URL Encode',             category: 'Encoding',    needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'UrlDecode',          label: 'URL Decode',             category: 'Encoding',    needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'HtmlEncode',         label: 'HTML Encode',            category: 'Encoding',    needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'HtmlDecode',         label: 'HTML Decode',            category: 'Encoding',    needsEncoding: false, needsEndian: false, needsByteCount: false },
	// Bytes
	{ value: 'StringToBytes',      label: 'String → Bytes',         category: 'Bytes',       needsEncoding: true,  needsEndian: false, needsByteCount: false },
	{ value: 'BytesToString',      label: 'Bytes → String',         category: 'Bytes',       needsEncoding: true,  needsEndian: false, needsByteCount: false },
	{ value: 'IntToBytes',         label: 'Int → Bytes',            category: 'Bytes',       needsEncoding: false, needsEndian: true,  needsByteCount: true  },
	{ value: 'BytesToInt',         label: 'Bytes → Int',            category: 'Bytes',       needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'BigIntToBytes',      label: 'BigInt → Bytes',         category: 'Bytes',       needsEncoding: false, needsEndian: true,  needsByteCount: false },
	{ value: 'BytesToBigInt',      label: 'Bytes → BigInt',         category: 'Bytes',       needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'BytesToBinaryString',label: 'Bytes → Binary String',  category: 'Bytes',       needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'BinaryStringToBytes',label: 'Binary String → Bytes',  category: 'Bytes',       needsEncoding: false, needsEndian: false, needsByteCount: false },
	// Formatting
	{ value: 'ReadableSize',       label: 'Readable Size',          category: 'Formatting',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'NumberToWords',      label: 'Number → Words',         category: 'Formatting',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'WordsToNumber',      label: 'Words → Number',         category: 'Formatting',  needsEncoding: false, needsEndian: false, needsByteCount: false },
	{ value: 'SvgToPng',           label: 'SVG → PNG (base64)',     category: 'Rendering',   needsEncoding: false, needsEndian: false, needsByteCount: false },
];

export function getConversionMeta(op: string) {
	return CONVERSION_OPS.find(o => o.value === op) || CONVERSION_OPS[0];
}

export const FILE_SYSTEM_OPS = [
	{ value: 'CreatePath',       label: 'Create Path',           hasContent: false, hasDest: false, hasOutput: false },
	{ value: 'FileAppend',       label: 'File Append',           hasContent: true,  hasDest: false, hasOutput: false },
	{ value: 'FileAppendLines',  label: 'File Append Lines',     hasContent: true,  hasDest: false, hasOutput: false },
	{ value: 'FileCopy',         label: 'File Copy',             hasContent: false, hasDest: true,  hasOutput: false },
	{ value: 'FileMove',         label: 'File Move',             hasContent: false, hasDest: true,  hasOutput: false },
	{ value: 'FileDelete',       label: 'File Delete',           hasContent: false, hasDest: false, hasOutput: false },
	{ value: 'FileExists',       label: 'File Exists',           hasContent: false, hasDest: false, hasOutput: true  },
	{ value: 'FileRead',         label: 'File Read',             hasContent: false, hasDest: false, hasOutput: true  },
	{ value: 'FileReadBytes',    label: 'File Read Bytes',       hasContent: false, hasDest: false, hasOutput: true  },
	{ value: 'FileReadLines',    label: 'File Read Lines',       hasContent: false, hasDest: false, hasOutput: true  },
	{ value: 'FileWrite',        label: 'File Write',            hasContent: true,  hasDest: false, hasOutput: false },
	{ value: 'FileWriteBytes',   label: 'File Write Bytes',      hasContent: true,  hasDest: false, hasOutput: false },
	{ value: 'FileWriteLines',   label: 'File Write Lines',      hasContent: true,  hasDest: false, hasOutput: false },
	{ value: 'FolderDelete',     label: 'Folder Delete',         hasContent: false, hasDest: false, hasOutput: false },
	{ value: 'FolderExists',     label: 'Folder Exists',         hasContent: false, hasDest: false, hasOutput: true  },
	{ value: 'GetFilesInFolder', label: 'Get Files in Folder',   hasContent: false, hasDest: false, hasOutput: true  },
];

export function getFileSystemMeta(op: string) {
	return FILE_SYSTEM_OPS.find(o => o.value === op) || FILE_SYSTEM_OPS[0];
}
