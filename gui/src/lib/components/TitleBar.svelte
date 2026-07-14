<script lang="ts">
	import { onMount } from 'svelte';
	import { send } from "$lib/ipc";
	import { app, requestAppClose } from "$lib/state.svelte";
	import Minus from '@lucide/svelte/icons/minus';
	import Square from '@lucide/svelte/icons/square';
	import X from '@lucide/svelte/icons/x';

	const TITLE_BAR_H = 28;
	const CHROME_BTN_W = 114; // 3 buttons × 38px

	// Position-based drag: if pointer is in the top 28px and left of the
	// chrome buttons zone, initiate native drag. Uses capture phase so it
	// fires before any dialog/overlay handlers. Purely coordinate-based so
	// dialog backdrops can never interfere — we check WHERE, not WHAT.
	onMount(() => {
		function onPointerDown(e: PointerEvent) {
			if (e.clientY <= TITLE_BAR_H && e.clientX < window.innerWidth - CHROME_BTN_W) {
				e.preventDefault();
				send('drag');
			}
		}
		// Listen for native close (Alt+F4, taskbar close) — redirect through unsaved-tab flow
		function onNativeClose() {
			requestAppClose();
		}
		window.addEventListener('pointerdown', onPointerDown, true);
		window.addEventListener('native-close-requested', onNativeClose);
		return () => {
			window.removeEventListener('pointerdown', onPointerDown, true);
			window.removeEventListener('native-close-requested', onNativeClose);
		};
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="fixed top-0 left-0 right-0 flex items-center bg-surface h-7 select-none panel-raised z-[9999]"
>
	<span class="flex-1 px-2 text-[11px] text-muted-foreground tracking-wide flex items-center gap-1.5">
		<img src="/ironbullet-logo.png" alt="" aria-hidden="true" draggable="false" class="h-3 w-3 rounded-[3px]" />
		Ironbullet
		<span class="text-muted-foreground/50 text-[10px]">{app.updateCurrentVersion}</span>
	</span>
	<div class="chrome-btns flex h-full">
		<button
			class="w-[38px] h-full flex items-center justify-center text-muted-foreground hover:bg-[#3e3e3e] hover:text-foreground transition-colors cursor-pointer"
			onclick={() => send('minimize')}
			title="Minimize"
		>
			<Minus size={12} />
		</button>
		<button
			class="w-[38px] h-full flex items-center justify-center text-muted-foreground hover:bg-[#3e3e3e] hover:text-foreground transition-colors cursor-pointer"
			onclick={() => send('maximize')}
			title="Maximize"
		>
			<Square size={10} />
		</button>
		<button
			class="w-[38px] h-full flex items-center justify-center text-muted-foreground hover:bg-red hover:text-white transition-colors cursor-pointer"
			onclick={requestAppClose}
			title="Close"
		>
			<X size={12} />
		</button>
	</div>
</div>
