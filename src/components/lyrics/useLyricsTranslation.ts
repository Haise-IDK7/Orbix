import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { translateOnDevice } from "./onDeviceTranslate";

/** "device" = built-in on-device translator, otherwise the cloud engine used. */
export type TranslationEngine = "device" | "microsoft" | "mymemory";

interface Translation {
	lines: string[];
	source: string;
	engine: TranslationEngine;
}

export type TranslationState =
	| { status: "off" | "loading" }
	| { status: "same"; source: string }
	| { status: "ready"; lines: string[]; engine: TranslationEngine }
	| { status: "error"; message: string };

/** The whole translation gives up after this, so the button never hangs. */
const TOTAL_TIMEOUT_MS = 60000;

/**
 * Line-by-line translation of the current lyrics while `enabled`, through the
 * backend (Microsoft Translator with the user's key, else MyMemory), which
 * batches, caches on disk and skips songs already in `target`. With
 * `onDevice`, the translator built into WebView2 is tried first (experimental:
 * some WebView2 versions expose it but never finish loading its model).
 */
export function useLyricsTranslation(
	lines: string[],
	enabled: boolean,
	target: string,
	onDevice = false
): TranslationState {
	const key = useMemo(() => lines.join("\n"), [lines]);
	const [state, setState] = useState<TranslationState>({ status: "off" });

	useEffect(() => {
		if (!enabled || !key.trim()) {
			setState({ status: "off" });
			return;
		}
		let cancelled = false;
		setState({ status: "loading" });
		const all = key.split("\n");
		let timer = 0;
		const timeout = new Promise<never>((_, reject) => {
			timer = window.setTimeout(
				() => reject(new Error("Translation took too long. Click to try again.")),
				TOTAL_TIMEOUT_MS
			);
		});
		Promise.race([
			(async (): Promise<Translation> => {
				const local = onDevice ? await translateOnDevice(all, target) : null;
				if (local) return { ...local, engine: "device" };
				return invoke<Translation>("translate_lyrics", { lines: all, target });
			})(),
			timeout
		])
			.then((t) => {
				if (cancelled) return;
				setState(
					t.source === target
						? { status: "same", source: t.source }
						: { status: "ready", lines: t.lines, engine: t.engine || "mymemory" }
				);
			})
			.catch(
				(e) =>
					!cancelled &&
					setState({ status: "error", message: e instanceof Error ? e.message : String(e) })
			)
			.finally(() => window.clearTimeout(timer));
		return () => {
			cancelled = true;
			window.clearTimeout(timer);
		};
	}, [key, enabled, target, onDevice]);

	return state;
}
