import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { findActiveLine, parseLrc, type LyricLine } from "../components/lyrics/lrc";

export type LyricsStatus =
	| "idle"
	| "loading"
	| "synced"
	| "plain"
	| "instrumental"
	| "notfound"
	| "error";

interface LyricsResult {
	synced: string | null;
	plain: string | null;
	instrumental: boolean;
}

export interface LyricsState {
	status: LyricsStatus;
	lines: LyricLine[];
	plainText: string;
}

const IDLE: LyricsState = { status: "idle", lines: [], plainText: "" };
// Players often publish the title before the artist/duration; wait for the
// metadata to settle so the lookup uses the final values.
const METADATA_SETTLE_MS = 400;

/** Fetches lyrics for the current track while `enabled`. */
export function useLyrics(
	enabled: boolean,
	title: string,
	artist: string,
	durationMs: number
): LyricsState {
	const [state, setState] = useState<LyricsState>(IDLE);
	const durationRef = useRef(durationMs);
	durationRef.current = durationMs;

	useEffect(() => {
		if (!enabled || !title.trim()) {
			setState(IDLE);
			return;
		}
		let cancelled = false;
		setState({ status: "loading", lines: [], plainText: "" });
		const timer = setTimeout(async () => {
			try {
				const result = await invoke<LyricsResult | null>("fetch_lyrics", {
					title,
					artist,
					durationMs: Math.round(durationRef.current || 0)
				});
				if (cancelled) return;
				if (!result) {
					setState({ status: "notfound", lines: [], plainText: "" });
				} else if (result.synced) {
					const lines = parseLrc(result.synced);
					setState(
						lines.length > 0
							? { status: "synced", lines, plainText: result.plain ?? "" }
							: { status: "plain", lines: [], plainText: result.plain ?? result.synced }
					);
				} else if (result.plain) {
					setState({ status: "plain", lines: [], plainText: result.plain });
				} else {
					setState({
						status: result.instrumental ? "instrumental" : "notfound",
						lines: [],
						plainText: ""
					});
				}
			} catch (err) {
				console.error("Failed to fetch lyrics:", err);
				if (!cancelled) setState({ status: "error", lines: [], plainText: "" });
			}
		}, METADATA_SETTLE_MS);
		return () => {
			cancelled = true;
			clearTimeout(timer);
		};
	}, [enabled, title, artist]);

	return state;
}

interface PlaybackClock {
	positionMs: number;
	updatedAt: number;
	isPlaying: boolean;
}

/**
 * Index of the line being sung, re-evaluated every 100 ms from the backend's
 * position + timestamp. Only triggers a re-render when the line changes.
 */
export function useActiveLyricIndex(lines: LyricLine[], clock: PlaybackClock): number {
	const [index, setIndex] = useState(-1);
	const { positionMs, updatedAt, isPlaying } = clock;

	useEffect(() => {
		if (lines.length === 0) {
			setIndex(-1);
			return;
		}
		const tick = () => {
			const elapsed = isPlaying && updatedAt > 0 ? Date.now() - updatedAt : 0;
			setIndex(findActiveLine(lines, positionMs + Math.max(0, elapsed)));
		};
		tick();
		if (!isPlaying) return;
		const interval = setInterval(tick, 100);
		return () => clearInterval(interval);
	}, [lines, positionMs, updatedAt, isPlaying]);

	return index;
}
