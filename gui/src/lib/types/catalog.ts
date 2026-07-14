import type { BlockType } from './pipeline';
import type { BlockMeta } from './ui';

export const BLOCK_CATALOG: BlockMeta[] = [
	{ type: 'HttpRequest', label: 'HTTP Request', category: 'Requests', color: '#0078d4', icon: 'globe' },
	{ type: 'Parse', label: 'Parse', category: 'Parsing', color: '#4ec9b0', icon: 'scissors' },
	{ type: 'KeyCheck', label: 'Key Check', category: 'Checks', color: '#d7ba7d', icon: 'shield-check' },
	{ type: 'StringFunction', label: 'String Function', category: 'Functions', color: '#c586c0', icon: 'type' },
	{ type: 'ListFunction', label: 'List Function', category: 'Functions', color: '#c586c0', icon: 'list' },
	{ type: 'CryptoFunction', label: 'Crypto Function', category: 'Functions', color: '#c586c0', icon: 'lock' },
	{ type: 'ConversionFunction', label: 'Conversion', category: 'Functions', color: '#c586c0', icon: 'arrow-right-left' },
	{ type: 'DateFunction', label: 'Date Function', category: 'Functions', color: '#c586c0', icon: 'calendar' },
	{ type: 'CookieContainer', label: 'Cookie Container', category: 'Functions', color: '#c586c0', icon: 'cookie' },
	{ type: 'ByteArray', label: 'Byte Array', category: 'Functions', color: '#c586c0', icon: 'file-type' },
	{ type: 'Constants', label: 'Constants', category: 'Functions', color: '#c586c0', icon: 'database' },
	{ type: 'Dictionary', label: 'Dictionary', category: 'Functions', color: '#c586c0', icon: 'book-open' },
	{ type: 'FloatFunction', label: 'Float Function', category: 'Functions', color: '#c586c0', icon: 'calculator' },
	{ type: 'IntegerFunction', label: 'Integer Function', category: 'Functions', color: '#c586c0', icon: 'hash' },
	{ type: 'TimeFunction', label: 'Time Function', category: 'Functions', color: '#c586c0', icon: 'clock' },
	{ type: 'GenerateGUID', label: 'Generate GUID', category: 'Functions', color: '#c586c0', icon: 'fingerprint' },
	{ type: 'PhoneCountry', label: 'Phone Country', category: 'Functions', color: '#c586c0', icon: 'phone' },
	{ type: 'IfElse', label: 'If / Else', category: 'Control', color: '#dcdcaa', icon: 'git-branch' },
	{ type: 'Loop', label: 'Loop', category: 'Control', color: '#dcdcaa', icon: 'repeat' },
	{ type: 'Delay', label: 'Delay', category: 'Control', color: '#dcdcaa', icon: 'clock' },
	{ type: 'CaseSwitch', label: 'Case / Switch', category: 'Control', color: '#dcdcaa', icon: 'list-tree' },
	{ type: 'Log', label: 'Log', category: 'Utilities', color: '#858585', icon: 'file-text' },
	{ type: 'SetVariable', label: 'Set Variable', category: 'Utilities', color: '#858585', icon: 'variable' },
	{ type: 'ClearCookies', label: 'Clear Cookies', category: 'Utilities', color: '#858585', icon: 'cookie' },
	{ type: 'Webhook', label: 'Webhook', category: 'Utilities', color: '#858585', icon: 'globe' },
	// Protocol requests
	{ type: 'TcpRequest', label: 'TCP Request', category: 'Requests', color: '#0078d4', icon: 'cable' },
	{ type: 'UdpRequest', label: 'UDP Request', category: 'Requests', color: '#0078d4', icon: 'radio' },
	{ type: 'FtpRequest', label: 'FTP Request', category: 'Requests', color: '#0078d4', icon: 'hard-drive-download' },
	{ type: 'SshRequest', label: 'SSH Request', category: 'Requests', color: '#0078d4', icon: 'terminal' },
	{ type: 'ImapRequest', label: 'IMAP Request', category: 'Requests', color: '#0078d4', icon: 'mail' },
	{ type: 'SmtpRequest', label: 'SMTP Request', category: 'Requests', color: '#0078d4', icon: 'send' },
	{ type: 'PopRequest', label: 'POP Request', category: 'Requests', color: '#0078d4', icon: 'inbox' },
	// Bypass / Anti-bot
	{ type: 'CaptchaSolver', label: 'Captcha Solver', category: 'Bypass', color: '#e5c07b', icon: 'shield' },
	{ type: 'CloudflareBypass', label: 'Cloudflare Bypass', category: 'Bypass', color: '#e5c07b', icon: 'cloud' },
	{ type: 'LaravelCsrf', label: 'Laravel CSRF', category: 'Bypass', color: '#e5c07b', icon: 'key' },
	// New blocks
	{ type: 'RandomUserAgent', label: 'Random User Agent', category: 'Utilities', color: '#858585', icon: 'user' },
	{ type: 'OcrCaptcha', label: 'OCR Captcha', category: 'Bypass', color: '#e5c07b', icon: 'scan-eye' },
	{ type: 'RecaptchaInvisible', label: 'reCAPTCHA Invisible', category: 'Bypass', color: '#e5c07b', icon: 'shield-check' },
	{ type: 'XacfSensor', label: 'XACF Sensor', category: 'Sensors', color: '#2dd4bf', icon: 'cpu' },
	{ type: 'DataDomeSensor', label: 'DataDome Sensor', category: 'Sensors', color: '#2dd4bf', icon: 'cpu' },
	{ type: 'AkamaiV3Sensor', label: 'Akamai V3 Sensor', category: 'Sensors', color: '#2dd4bf', icon: 'cpu' },
	// Security
	{ type: 'JwtToken', label: 'JWT Token', category: 'Security', color: '#a78bfa', icon: 'key-round' },
	{ type: 'HeaderSpoof', label: 'Header Spoof', category: 'Security', color: '#a78bfa', icon: 'shield-alert' },
	{ type: 'RandomData', label: 'Random Data', category: 'Utilities', color: '#858585', icon: 'dices' },
	{ type: 'Plugin', label: 'Plugin Block', category: 'Utilities', color: '#858585', icon: 'plug' },
	{ type: 'Group', label: 'Group', category: 'Control', color: '#dcdcaa', icon: 'folder' },

	{ type: 'FileSystem', label: 'File System', category: 'FileSystem', color: '#d4a96a', icon: 'folder-open' },
	// Browser automation
	{ type: 'BrowserOpen', label: 'Browser Open', category: 'Browser', color: '#e06c75', icon: 'monitor' },
	{ type: 'NavigateTo', label: 'Navigate To', category: 'Browser', color: '#e06c75', icon: 'globe' },
	{ type: 'ClickElement', label: 'Click Element', category: 'Browser', color: '#e06c75', icon: 'mouse-pointer-click' },
	{ type: 'TypeText', label: 'Type Text', category: 'Browser', color: '#e06c75', icon: 'keyboard' },
	{ type: 'WaitForElement', label: 'Wait For Element', category: 'Browser', color: '#e06c75', icon: 'hourglass' },
	{ type: 'GetElementText', label: 'Get Element Text', category: 'Browser', color: '#e06c75', icon: 'scan-text' },
	{ type: 'Screenshot', label: 'Screenshot', category: 'Browser', color: '#e06c75', icon: 'camera' },
	{ type: 'ExecuteJs', label: 'Execute JS', category: 'Browser', color: '#e06c75', icon: 'terminal' },
];

export function getBlockCategory(type: BlockType): string {
	return BLOCK_CATALOG.find(b => b.type === type)?.category || 'Utilities';
}

export function getBlockColor(type: BlockType): string {
	return BLOCK_CATALOG.find(b => b.type === type)?.color || '#858585';
}

export function getBlockCssClass(type: BlockType): string {
	const cat = getBlockCategory(type);
	switch (cat) {
		case 'Requests': return 'block-request';
		case 'Parsing': return 'block-parse';
		case 'Checks': return 'block-check';
		case 'Functions': return 'block-function';
		case 'Control': return 'block-control';
		case 'Utilities': return 'block-utility';
		case 'Bypass': return 'block-bypass';
		case 'Security': return 'block-bypass';
		case 'Browser': return 'block-browser';
		case 'Sensors': return 'block-sensor';
		case 'Data': return 'block-data';
		case 'FileSystem': return 'block-data';
		default: return 'block-utility';
	}
}
