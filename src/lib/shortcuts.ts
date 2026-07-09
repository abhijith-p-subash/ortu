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
	openPopup: string; // ⌥V   / Alt+V
	pasteStack: string; // ⌥⇧V  / Alt+Shift+V
	addToStack: string; // ⌘S   / Ctrl+S       (in-app: selected history item)
	copyToStack: string; // ⌘⇧C / Ctrl+Shift+C (global: live selection in any app)
}

export function getNamedShortcuts(platform: string): NamedShortcuts {
	const k = getKeyLabels(platform);
	return {
		openPopup: combo(platform, k.alt, "V"),
		pasteStack: combo(platform, k.alt, k.shift, "V"),
		addToStack: combo(platform, k.mod, "S"),
		copyToStack: combo(platform, k.mod, k.shift, "C"),
	};
}

// ── User-rebindable GLOBAL shortcut actions ─────────────────────────────────
// These ids mirror crate::SHORTCUT_ACTIONS in the Rust backend.

export interface ShortcutActionMeta {
	id: string;
	label: string;
	description: string;
}

export const SHORTCUT_ACTIONS: ShortcutActionMeta[] = [
	{
		id: "open_popup",
		label: "Open quick popup",
		description: "Show the quick-access popup from any app",
	},
	{
		id: "copy_stack",
		label: "Copy selection to stack",
		description: "Queue your current selection from any app",
	},
	{
		id: "paste_stack",
		label: "Paste next from stack",
		description: "Paste the next queued item, in order",
	},
];

/** Renders a Tauri accelerator string (e.g. "CommandOrControl+Shift+C") into an
 *  OS-native label (⌘⇧C on macOS, Ctrl+Shift+C elsewhere). */
export function prettyAccelerator(acc: string, platform: string): string {
	if (!acc) return "—";
	const isMac = isMacPlatform(platform);
	const tokens = acc
		.split("+")
		.map((t) => t.trim())
		.filter(Boolean)
		.map((t) => {
			switch (t.toLowerCase()) {
				case "commandorcontrol":
				case "cmdorctrl":
					return isMac ? "⌘" : "Ctrl";
				case "command":
				case "cmd":
				case "super":
				case "meta":
					return isMac ? "⌘" : "Super";
				case "control":
				case "ctrl":
					return isMac ? "⌃" : "Ctrl";
				case "alt":
				case "option":
					return isMac ? "⌥" : "Alt";
				case "shift":
					return isMac ? "⇧" : "Shift";
				case "up":
					return "↑";
				case "down":
					return "↓";
				case "left":
					return "←";
				case "right":
					return "→";
				default:
					return t.length === 1 ? t.toUpperCase() : t;
			}
		});
	return tokens.join(isMac ? "" : "+");
}

const NON_PRINTABLE: Record<string, string> = {
	Space: "Space",
	Enter: "Enter",
	Tab: "Tab",
	Backspace: "Backspace",
	Delete: "Delete",
	ArrowUp: "Up",
	ArrowDown: "Down",
	ArrowLeft: "Left",
	ArrowRight: "Right",
	Comma: ",",
	Period: ".",
	Slash: "/",
	Backquote: "`",
	Minus: "-",
	Equal: "=",
	BracketLeft: "[",
	BracketRight: "]",
	Semicolon: ";",
	Quote: "'",
	Backslash: "\\",
};

const MODIFIER_KEY_NAMES = new Set([
	"Shift",
	"Control",
	"Alt",
	"Meta",
	"CapsLock",
	"ContextMenu",
]);

/** Builds a Tauri accelerator string from a keydown event, or null if the
 *  combo is incomplete (no main key, or no modifier). */
export function acceleratorFromEvent(e: KeyboardEvent, platform = ""): string | null {
	if (MODIFIER_KEY_NAMES.has(e.key)) return null; // only modifiers held so far

	const code = e.code;
	let main = "";
	if (/^Key[A-Z]$/.test(code)) main = code.slice(3);
	else if (/^Digit\d$/.test(code)) main = code.slice(5);
	else if (/^Numpad\d$/.test(code)) main = code.slice(6);
	else if (/^F\d{1,2}$/.test(code)) main = code;
	else if (NON_PRINTABLE[code]) main = NON_PRINTABLE[code];

	if (!main) return null;

	const mods: string[] = [];
	// Meta is ⌘ on macOS (the primary modifier) but the Win/Super key on
	// Windows & Linux, where it must not be folded into Ctrl.
	const isMac = isMacPlatform(platform);
	if (e.metaKey && !isMac) mods.push("Super");
	if (e.ctrlKey || (e.metaKey && isMac)) mods.push("CommandOrControl");
	if (e.altKey) mods.push("Alt");
	if (e.shiftKey) mods.push("Shift");
	if (mods.length === 0) return null; // global shortcuts require a modifier

	return [...mods, main].join("+");
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
				{ label: "Copy selection to stack (any app)", keys: c(k.mod, k.shift, "C") },
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
				{ label: "Add selected item to stack", keys: c(k.mod, "S") },
				{ label: "Manage groups", keys: c(k.mod, "G") },
				{ label: "Save (in editor)", keys: c(k.mod, k.enter) },
			],
		},
	];
}
