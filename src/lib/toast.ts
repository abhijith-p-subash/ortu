import { writable } from "svelte/store";

export type ToastType = "success" | "error" | "info";

export interface Toast {
	id: number;
	message: string;
	type: ToastType;
}

export const toasts = writable<Toast[]>([]);

let counter = 0;
const MAX_VISIBLE = 4;

/** Show a toast. Errors linger longer; success/info auto-dismiss quickly. */
export function showToast(message: string, type: ToastType = "info", duration?: number): void {
	const id = ++counter;
	toasts.update((list) => {
		const next = [...list, { id, message, type }];
		// Keep the stack tidy — drop the oldest if too many pile up.
		return next.length > MAX_VISIBLE ? next.slice(next.length - MAX_VISIBLE) : next;
	});
	const ms = duration ?? (type === "error" ? 4000 : 2200);
	setTimeout(() => dismissToast(id), ms);
}

export function dismissToast(id: number): void {
	toasts.update((list) => list.filter((t) => t.id !== id));
}
