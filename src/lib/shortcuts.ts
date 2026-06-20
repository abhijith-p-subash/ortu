// Single source of truth for keyboard shortcuts.
// Labels adapt to the operating system so the UI shows OS-native keys
// (⌘ ⌥ ⌫ on macOS, Ctrl / Alt / Backspace on Windows & Linux).

export interface KeyLabels {
	mod: string; // primary modifier:   ⌘ / Ctrl
	alt: string; // secondary modifier: ⌥ / Alt
	shift: string; // ⇧ / Shift
	enter: string; // ↵
	del: string; // ⌫ / Backspace
	esc: string; // Esc
	up: string; // ↑
	down: string; // ↓
}

export function isMacPlatform(platform: string): boolean {
	return platform === "macos";
}

export function getKeyLabels(platform: string): KeyLabels {
	const isMac = isMacPlatform(platform);
	return {
		mod: isMac ? "⌘" : "Ctrl",
		alt: isMac ? "⌥" : "Alt",
		shift: isMac ? "⇧" : "Shift",
		enter: "↵",
		del: isMac ? "⌫" : "Backspace",
		esc: "Esc",
		up: "↑",
		down: "↓",
	};
}

/** Join modifier + key the OS-native way: "⌘C" on macOS, "Ctrl+C" elsewhere. */
export function combo(platform: string, ...parts: string[]): string {
	return parts.join(isMacPlatform(platform) ? "" : "+");
}

/** Named global shortcuts, formatted OS-natively, for use in inline UI hints. */
export interface NamedShortcuts {
	openPopup: string; // ⌥V  / Alt+V
	pasteStack: string; // ⌥⇧V / Alt+Shift+V
}

export function getNamedShortcuts(platform: string): NamedShortcuts {
	const k = getKeyLabels(platform);
	return {
		openPopup: combo(platform, k.alt, "V"),
		pasteStack: combo(platform, k.alt, k.shift, "V"),
	};
}

export interface ShortcutItem {
	label: string;
	keys: string;
}

export interface ShortcutSection {
	title: string;
	items: ShortcutItem[];
}

export function getShortcutSections(platform: string): ShortcutSection[] {
	const k = getKeyLabels(platform);
	const c = (...parts: string[]) => combo(platform, ...parts);

	return [
		{
			title: "Global",
			items: [
				{ label: "Open quick popup (anywhere)", keys: c(k.alt, "V") },
				{ label: "Paste next item from stack", keys: c(k.alt, k.shift, "V") },
			],
		},
		{
			title: "Navigation",
			items: [
				{ label: "Move selection", keys: `${k.up} / ${k.down}` },
				{ label: "Copy selected item", keys: `${k.enter} / Click` },
				{ label: "Quick copy by position", keys: c(k.mod, "1–9") },
				{ label: "Close window", keys: k.esc },
			],
		},
		{
			title: "Item actions",
			items: [
				{ label: "Delete item", keys: `Del / ${c(k.mod, k.del)}` },
				{ label: "Pin / Unpin", keys: c(k.mod, "P") },
				{ label: "Add to group", keys: c(k.mod, "C") },
				{ label: "Manage groups", keys: c(k.mod, "G") },
				{ label: "Save (in editor)", keys: c(k.mod, k.enter) },
			],
		},
	];
}
