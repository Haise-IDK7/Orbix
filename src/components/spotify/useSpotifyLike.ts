import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useStoredSetting } from "../../hooks/useStoredSetting";

interface TrackState {
	track_id: string;
	liked: boolean;
}

/** Spotify can report the new track a moment after the media session does. */
const LOOKUP_DELAYS_MS = [800, 4000];

/**
 * Like state of the song in the notch, when it is playing on Spotify and the
 * account is connected in Settings. `available` is false otherwise, so the
 * heart only appears when it can actually work.
 */
export function useSpotifyLike(title: string, active: boolean) {
	const [connected] = useStoredSetting("bloom-spotify-connected", "false");
	const [state, setState] = useState<TrackState | null>(null);

	useEffect(() => {
		setState(null);
		if (!active || connected !== "true" || !title.trim()) return;
		let cancelled = false;
		const timers = LOOKUP_DELAYS_MS.map((delay, attempt) =>
			window.setTimeout(() => {
				invoke<TrackState | null>("spotify_track_state", { title })
					.then((s) => {
						if (cancelled) return;
						// Keep a found track rather than clearing it on a late retry.
						if (s || attempt === 0) setState((prev) => s ?? prev);
					})
					.catch(() => {});
			}, delay)
		);
		return () => {
			cancelled = true;
			timers.forEach(window.clearTimeout);
		};
	}, [title, active, connected]);

	const toggle = useCallback(() => {
		if (!state) return;
		const next = !state.liked;
		setState({ ...state, liked: next });
		invoke("spotify_set_liked", { trackId: state.track_id, liked: next }).catch((e) => {
			console.error("Spotify like failed:", e);
			setState((s) => (s ? { ...s, liked: !next } : s));
		});
	}, [state]);

	return { available: state !== null, liked: state?.liked ?? false, toggle };
}
