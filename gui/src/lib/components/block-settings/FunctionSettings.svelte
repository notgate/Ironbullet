<script lang="ts">
	import type { Block } from '$lib/types';
	import SkeuSelect from '../SkeuSelect.svelte';
	import VariableInput from '../VariableInput.svelte';
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import {
		inputCls, labelCls,
		STRING_FUNCTIONS, getStringFuncMeta,
		LIST_FUNCTIONS, getListFuncMeta,
		CRYPTO_FUNCTIONS, getCryptoFuncMeta,
		CONVERSION_OPS, getConversionMeta,
		FILE_SYSTEM_OPS, getFileSystemMeta,
	} from './shared';

	let { block, updateSettings }: {
		block: Block;
		updateSettings: (key: string, value: unknown) => void;
	} = $props();
</script>

<!-- ===================== STRING FUNCTION ===================== -->
{#if block.settings.type === 'StringFunction'}
	{@const meta = getStringFuncMeta(block.settings.function_type)}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Function</label>
			<SkeuSelect value={block.settings.function_type}
				onValueChange={(v) => updateSettings('function_type', v)}
				options={STRING_FUNCTIONS.map(f => ({value: f.value, label: f.label}))}
				class="w-full mt-0.5"
			/>
		</div>
		<div>
			<label class={labelCls}>Input variable</label>
			<VariableInput value={block.settings.input_var} placeholder="data.SOURCE" class={inputCls}
				oninput={(e) => updateSettings('input_var', (e.target as HTMLInputElement).value)} />
		</div>
		{#if meta.p1}
			<div>
				<label class={labelCls}>{meta.p1}</label>
				<VariableInput value={block.settings.param1} class={inputCls}
					oninput={(e) => updateSettings('param1', (e.target as HTMLInputElement).value)} />
			</div>
		{/if}
		{#if meta.p2}
			<div>
				<label class={labelCls}>{meta.p2}</label>
				<VariableInput value={block.settings.param2} class={inputCls}
					oninput={(e) => updateSettings('param2', (e.target as HTMLInputElement).value)} />
			</div>
		{/if}
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

<!-- ===================== TRANSLATE ===================== -->
{:else if block.settings.type === 'Translate'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Input variable</label>
			<VariableInput value={block.settings.input_var} placeholder="data.SOURCE" class={inputCls}
				oninput={(e) => updateSettings('input_var', (e.target as HTMLInputElement).value)} />
		</div>
		<div>
			<label class={labelCls}>Mappings</label>
			<textarea value={block.settings.mappings} placeholder={'pending => PENDING\nactive => ACTIVE\n# unmatched values pass through'}
				class="w-full skeu-input text-[11px] font-mono min-h-[120px] resize-y mt-0.5"
				oninput={(e) => updateSettings('mappings', (e.target as HTMLTextAreaElement).value)}></textarea>
			<p class="text-[9px] text-muted-foreground mt-0.5">One exact <code class="text-foreground/70">source =&gt; destination</code> mapping per line. Empty and <code class="text-foreground/70">#</code> comment lines are ignored; unmatched input is preserved.</p>
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

<!-- ===================== LIST FUNCTION ===================== -->
{:else if block.settings.type === 'ListFunction'}
	{@const meta = getListFuncMeta(block.settings.function_type)}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Function</label>
			<SkeuSelect value={block.settings.function_type}
				onValueChange={(v) => updateSettings('function_type', v)}
				options={LIST_FUNCTIONS.map(f => ({value: f.value, label: f.label}))}
				class="w-full mt-0.5"
			/>
		</div>
		<div>
			<label class={labelCls}>Input variable (list)</label>
			<VariableInput value={block.settings.input_var} placeholder="@myList" class={inputCls}
				oninput={(e) => updateSettings('input_var', (e.target as HTMLInputElement).value)} />
		</div>
		{#if meta.param}
			<div>
				<label class={labelCls}>{meta.param}</label>
				<VariableInput value={block.settings.param1} class={inputCls}
					oninput={(e) => updateSettings('param1', (e.target as HTMLInputElement).value)} />
			</div>
		{/if}
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

<!-- ===================== CRYPTO FUNCTION ===================== -->
{:else if block.settings.type === 'CryptoFunction'}
	{@const meta = getCryptoFuncMeta(block.settings.function_type)}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Algorithm</label>
			<SkeuSelect value={block.settings.function_type}
				onValueChange={(v) => updateSettings('function_type', v)}
				options={CRYPTO_FUNCTIONS.map(f => ({value: f.value, label: f.label}))}
				class="w-full mt-0.5"
			/>
		</div>
		<div>
			<label class={labelCls}>Input variable</label>
			<VariableInput value={block.settings.input_var} placeholder="data.SOURCE" class={inputCls}
				oninput={(e) => updateSettings('input_var', (e.target as HTMLInputElement).value)} />
		</div>
		{#if meta.needsKey}
			<div>
				<label class={labelCls}>Key / Secret</label>
				<VariableInput value={block.settings.key} class={inputCls}
					oninput={(e) => updateSettings('key', (e.target as HTMLInputElement).value)} />
			</div>
		{/if}
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

<!-- ===================== CONVERSION FUNCTION ===================== -->
{:else if block.settings.type === 'ConversionFunction'}
	{@const convMeta = getConversionMeta(block.settings.op)}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Operation</label>
			<SkeuSelect value={block.settings.op ?? 'Base64Encode'}
				onValueChange={(v) => updateSettings('op', v)}
				options={CONVERSION_OPS.map(o => ({ value: o.value, label: `[${o.category}] ${o.label}` }))}
				class="w-full mt-0.5"
			/>
		</div>
		<div>
			<label class={labelCls}>Input variable</label>
			<VariableInput value={block.settings.input_var} class={inputCls}
				oninput={(e) => updateSettings('input_var', (e.target as HTMLInputElement).value)} />
		</div>
		{#if convMeta.needsEncoding}
			<div>
				<label class={labelCls}>Encoding</label>
				<SkeuSelect value={block.settings.encoding ?? 'utf8'}
					onValueChange={(v) => updateSettings('encoding', v)}
					options={[{value:'utf8',label:'UTF-8'},{value:'utf16',label:'UTF-16 BE'}]}
					class="w-full mt-0.5"
				/>
			</div>
		{/if}
		{#if convMeta.needsEndian}
			<div>
				<label class={labelCls}>Byte order</label>
				<SkeuSelect value={block.settings.endianness ?? 'big'}
					onValueChange={(v) => updateSettings('endianness', v)}
					options={[{value:'big',label:'Big-endian'},{value:'little',label:'Little-endian'}]}
					class="w-full mt-0.5"
				/>
			</div>
		{/if}
		{#if convMeta.needsByteCount}
			<div>
				<label class={labelCls}>Byte count (1–8)</label>
				<input type="number" min="1" max="8" value={block.settings.byte_count ?? 4}
					class={inputCls}
					oninput={(e) => updateSettings('byte_count', parseInt((e.target as HTMLInputElement).value) || 4)} />
			</div>
		{/if}
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

<!-- ===================== BYTE ARRAY ===================== -->
{:else if block.settings.type === 'ByteArray'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Operation</label>
			<SkeuSelect value={block.settings.operation}
				onValueChange={(v) => updateSettings('operation', v)}
				options={[
					{value: 'ToHex', label: 'To Hex'},
					{value: 'FromHex', label: 'From Hex'},
					{value: 'ToBase64', label: 'To Base64'},
					{value: 'FromBase64', label: 'From Base64'},
					{value: 'ToUtf8', label: 'To UTF-8'},
					{value: 'FromUtf8', label: 'From UTF-8'},
				]}
				class="w-full mt-0.5"
			/>
		</div>
		<div>
			<label class={labelCls}>Input variable</label>
			<VariableInput value={block.settings.input_var} class={inputCls}
				oninput={(e) => updateSettings('input_var', (e.target as HTMLInputElement).value)} />
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

<!-- ===================== CONSTANTS ===================== -->
{:else if block.settings.type === 'Constants'}
	<div class="space-y-1.5">
		<label class={labelCls}>Constants</label>
		{#each block.settings.constants as constant, i}
			<div class="flex gap-2">
				<VariableInput value={constant.name} placeholder="NAME" class={inputCls + ' flex-1'}
					oninput={(e) => {
						const updated = [...block.settings.constants];
						updated[i].name = (e.target as HTMLInputElement).value;
						updateSettings('constants', updated);
					}} />
				<VariableInput value={constant.value} placeholder="value" class={inputCls + ' flex-[2]'}
					oninput={(e) => {
						const updated = [...block.settings.constants];
						updated[i].value = (e.target as HTMLInputElement).value;
						updateSettings('constants', updated);
					}} />
				<button onclick={() => {
					const updated = block.settings.constants.filter((_, idx) => idx !== i);
					updateSettings('constants', updated);
				}} class="px-2 text-xs text-red-500 hover:text-red-400">✕</button>
			</div>
		{/each}
		<button onclick={() => {
			updateSettings('constants', [...block.settings.constants, {name: '', value: ''}]);
		}} class="text-xs text-blue-400 hover:text-blue-300">+ Add Constant</button>
	</div>

<!-- ===================== DICTIONARY ===================== -->
{:else if block.settings.type === 'Dictionary'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Operation</label>
			<SkeuSelect value={block.settings.operation}
				onValueChange={(v) => updateSettings('operation', v)}
				options={[
					{value: 'Get', label: 'Get'},
					{value: 'Set', label: 'Set'},
					{value: 'Remove', label: 'Remove'},
					{value: 'Exists', label: 'Exists'},
					{value: 'Keys', label: 'Keys'},
					{value: 'Values', label: 'Values'},
				]}
				class="w-full mt-0.5"
			/>
		</div>
		<div>
			<label class={labelCls}>Dictionary variable</label>
			<VariableInput value={block.settings.dict_var} class={inputCls}
				oninput={(e) => updateSettings('dict_var', (e.target as HTMLInputElement).value)} />
		</div>
		{#if ['Get', 'Set', 'Remove', 'Exists'].includes(block.settings.operation)}
			<div>
				<label class={labelCls}>Key</label>
				<VariableInput value={block.settings.key} class={inputCls}
					oninput={(e) => updateSettings('key', (e.target as HTMLInputElement).value)} />
			</div>
		{/if}
		{#if block.settings.operation === 'Set'}
			<div>
				<label class={labelCls}>Value</label>
				<VariableInput value={block.settings.value} class={inputCls}
					oninput={(e) => updateSettings('value', (e.target as HTMLInputElement).value)} />
			</div>
		{/if}
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

<!-- ===================== FLOAT FUNCTION ===================== -->
{:else if block.settings.type === 'FloatFunction'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Function</label>
			<SkeuSelect value={block.settings.function_type}
				onValueChange={(v) => updateSettings('function_type', v)}
				options={[
					{value: 'Round', label: 'Round'},
					{value: 'Ceil', label: 'Ceiling'},
					{value: 'Floor', label: 'Floor'},
					{value: 'Abs', label: 'Absolute'},
					{value: 'Add', label: 'Add'},
					{value: 'Subtract', label: 'Subtract'},
					{value: 'Multiply', label: 'Multiply'},
					{value: 'Divide', label: 'Divide'},
					{value: 'Power', label: 'Power'},
					{value: 'Sqrt', label: 'Square Root'},
					{value: 'Min', label: 'Minimum'},
					{value: 'Max', label: 'Maximum'},
				]}
				class="w-full mt-0.5"
			/>
		</div>
		<div>
			<label class={labelCls}>Input variable</label>
			<VariableInput value={block.settings.input_var} class={inputCls}
				oninput={(e) => updateSettings('input_var', (e.target as HTMLInputElement).value)} />
		</div>
		{#if ['Add', 'Subtract', 'Multiply', 'Divide', 'Power', 'Min', 'Max', 'Round'].includes(block.settings.function_type)}
			<div>
				<label class={labelCls}>{block.settings.function_type === 'Round' ? 'Decimal places' : 'Parameter'}</label>
				<VariableInput value={block.settings.param1} class={inputCls}
					oninput={(e) => updateSettings('param1', (e.target as HTMLInputElement).value)} />
			</div>
		{/if}
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

<!-- ===================== INTEGER FUNCTION ===================== -->
{:else if block.settings.type === 'IntegerFunction'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Function</label>
			<SkeuSelect value={block.settings.function_type}
				onValueChange={(v) => updateSettings('function_type', v)}
				options={[
					{value: 'Add', label: 'Add'},
					{value: 'Subtract', label: 'Subtract'},
					{value: 'Multiply', label: 'Multiply'},
					{value: 'Divide', label: 'Divide'},
					{value: 'Modulo', label: 'Modulo'},
					{value: 'Power', label: 'Power'},
					{value: 'Abs', label: 'Absolute'},
					{value: 'Min', label: 'Minimum'},
					{value: 'Max', label: 'Maximum'},
				]}
				class="w-full mt-0.5"
			/>
		</div>
		<div>
			<label class={labelCls}>Input variable</label>
			<VariableInput value={block.settings.input_var} class={inputCls}
				oninput={(e) => updateSettings('input_var', (e.target as HTMLInputElement).value)} />
		</div>
		{#if ['Add', 'Subtract', 'Multiply', 'Divide', 'Modulo', 'Power', 'Min', 'Max'].includes(block.settings.function_type)}
			<div>
				<label class={labelCls}>Parameter</label>
				<VariableInput value={block.settings.param1} class={inputCls}
					oninput={(e) => updateSettings('param1', (e.target as HTMLInputElement).value)} />
			</div>
		{/if}
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

<!-- ===================== TIME FUNCTION ===================== -->
{:else if block.settings.type === 'TimeFunction'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Function</label>
			<SkeuSelect value={block.settings.function_type}
				onValueChange={(v) => updateSettings('function_type', v)}
				options={[
					{value: 'ConvertTimezone', label: 'Convert Timezone'},
					{value: 'GetTimezone', label: 'Get Timezone'},
					{value: 'IsDST', label: 'Is DST'},
					{value: 'DurationBetween', label: 'Duration Between'},
					{value: 'AddDuration', label: 'Add Duration'},
					{value: 'SubtractDuration', label: 'Subtract Duration'},
				]}
				class="w-full mt-0.5"
			/>
		</div>
		<div>
			<label class={labelCls}>Input variable</label>
			<VariableInput value={block.settings.input_var} class={inputCls}
				oninput={(e) => updateSettings('input_var', (e.target as HTMLInputElement).value)} />
		</div>
		{#if block.settings.function_type === 'ConvertTimezone'}
			<div>
				<label class={labelCls}>Target timezone</label>
				<VariableInput value={block.settings.target_timezone} placeholder="America/New_York" class={inputCls}
					oninput={(e) => updateSettings('target_timezone', (e.target as HTMLInputElement).value)} />
			</div>
		{/if}
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

<!-- ===================== GENERATE GUID ===================== -->
{:else if block.settings.type === 'GenerateGUID'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>GUID Version</label>
			<SkeuSelect value={block.settings.guid_version}
				onValueChange={(v) => updateSettings('guid_version', v)}
				options={[
					{value: 'V1', label: 'V1 (Timestamp)'},
					{value: 'V4', label: 'V4 (Random)'},
					{value: 'V5', label: 'V5 (Hash)'},
				]}
				class="w-full mt-0.5"
			/>
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

<!-- ===================== PHONE COUNTRY ===================== -->
{:else if block.settings.type === 'PhoneCountry'}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Output format</label>
			<SkeuSelect value={block.settings.output_format}
				onValueChange={(v) => updateSettings('output_format', v)}
				options={[
					{value: 'CountryCode', label: 'Country Code (1, 44, etc.)'},
					{value: 'CountryName', label: 'Country Name'},
					{value: 'ISO2', label: 'ISO-2 Code'},
					{value: 'ISO3', label: 'ISO-3 Code'},
				]}
				class="w-full mt-0.5"
			/>
		</div>
		<div>
			<label class={labelCls}>Input variable (phone number)</label>
			<VariableInput value={block.settings.input_var} placeholder="+1-555-123-4567" class={inputCls}
				oninput={(e) => updateSettings('input_var', (e.target as HTMLInputElement).value)} />
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

<!-- ===================== FILE SYSTEM ===================== -->
{#if block.settings.type === 'FileSystem'}
	{@const fsOp = block.settings.op}
	{@const needsDest = ['FileCopy','FileMove'].includes(fsOp)}
	{@const needsContent = ['FileWrite','FileWriteBytes','FileWriteLines','FileAppend','FileAppendLines'].includes(fsOp)}
	{@const needsPath = true}
	<div class="space-y-1.5">
		<div>
			<label class={labelCls}>Operation</label>
			<SkeuSelect value={block.settings.op}
				onValueChange={(v) => updateSettings('op', v)}
				options={FILE_SYSTEM_OPS.map(o => ({ value: o.value, label: o.label }))}
				class="w-full mt-0.5"
			/>
		</div>
		{#if needsPath}
			<div>
				<label class={labelCls}>{needsDest ? 'Source path' : 'Path'}</label>
				<VariableInput value={block.settings.path} placeholder="/path/to/file.txt or <VAR>" class={inputCls}
					oninput={(e) => updateSettings('path', (e.target as HTMLInputElement).value)} />
				<p class="text-[9px] text-muted-foreground mt-0.5">Use <code class="font-mono">&lt;VARNAME&gt;</code> for dynamic paths from variables</p>
			</div>
		{/if}
		{#if needsDest}
			<div>
				<label class={labelCls}>Destination path</label>
				<VariableInput value={block.settings.dest_path} placeholder="/path/to/dest.txt or <VAR>" class={inputCls}
					oninput={(e) => updateSettings('dest_path', (e.target as HTMLInputElement).value)} />
			</div>
		{/if}
		{#if needsContent}
			<div>
				<label class={labelCls}>Content <span class="text-muted-foreground/60">(or variable name)</span></label>
				<textarea rows="3" value={block.settings.content}
					class="w-full skeu-input font-mono mt-0.5 resize-y text-xs"
					placeholder="Text to write, or a variable name like OUTPUT"
					oninput={(e) => updateSettings('content', (e.target as HTMLTextAreaElement).value)}
				></textarea>
				<p class="text-[9px] text-muted-foreground mt-0.5">If the value matches a variable name, that variable's value is used.</p>
			</div>
		{/if}
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
