import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Translation {
	lines: string[];
	source: string;
}

export type TranslationState =
	| { status: "off" | "loading" }
	| { status: "same"; source: string }
	| { status: "ready"; lines: string[] }
	| { status: "error"; message: string };

/**
 * Line-by-line translation of the current lyrics while `enabled`. The
 * backend batches, caches on disk and skips songs already in `target`.
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
		invoke<Translation>("translate_lyrics", { lines: key.split("\n"), target })
			.then((t) => {
				if (cancelled) return;
				setState(
					t.source === target
						? { status: "same", source: t.source }
						: { status: "ready", lines: t.lines }
				);
			})
			.catch((e) => !cancelled && setState({ status: "error", message: String(e) }));
		return () => {
			cancelled = true;
		};
	}, [key, enabled, target]);

	return state;
}
