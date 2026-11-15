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

/**
 * Line-by-line translation of the current lyrics while `enabled`: the
 * on-device translator when this WebView has one (no limit, private), else
 * the backend (Microsoft Translator with the user's key, or MyMemory), which
 * batches, caches on disk and skips songs already in `target`.
 */
export function useLyricsTranslation(
	lines: string[],
	enabled: boolean,
	target: string
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
		(async (): Promise<Translation> => {
			const local = await translateOnDevice(all, target);
			if (local) return { ...local, engine: "device" };
			return invoke<Translation>("translate_lyrics", { lines: all, target });
		})()
			.then((t) => {
				if (cancelled) return;
				setState(
					t.source === target
						? { status: "same", source: t.source }
						: { status: "ready", lines: t.lines, engine: t.engine || "mymemory" }
				);
			})
			.catch((e) => !cancelled && setState({ status: "error", message: String(e) }));
		return () => {
			cancelled = true;
		};
	}, [key, enabled, target]);

	return state;
}
