import { useCallback, useEffect, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useStoredSetting } from "../../hooks/useStoredSetting";

interface TrackState {
	track_id: string;
	liked: boolean;
}

/**
 * - `setup`: Spotify isn't connected yet (Settings → Notch → Spotify).
 * - `loading`: looking the song up.
 * - `unavailable`: connected, but this song isn't playing on Spotify (or the lookup failed).
 * - `ready`: the heart saves/removes the song.
 */
export type LikeStatus = "setup" | "loading" | "unavailable" | "ready";

/** Spotify can report the new track a moment after the media session does. */
const LOOKUP_DELAYS_MS = [800, 4000];

/**
 * Like state of the song in the notch. The heart is always shown while
 * something plays; when it can't work yet, `status` and `hint` say why and
 * clicking it leads to the fix (Settings, or a fresh lookup).
 */
export function useSpotifyLike(title: string, active: boolean) {
	const [connected] = useStoredSetting("bloom-spotify-connected", "false");
	const [state, setState] = useState<TrackState | null>(null);
	const [status, setStatus] = useState<LikeStatus>("setup");
	const [error, setError] = useState("");
	const [attempt, setAttempt] = useState(0);

	useEffect(() => {
		setState(null);
		setError("");
		if (connected !== "true") {
			setStatus("setup");
			return;
		}
		if (!active || !title.trim()) {
			setStatus("unavailable");
			return;
		}
		setStatus("loading");
		let cancelled = false;
		let found = false;
		const timers = LOOKUP_DELAYS_MS.map((delay, i) =>
			window.setTimeout(() => {
				if (found) return;
				invoke<TrackState | null>("spotify_track_state", { title })
					.then((s) => {
						if (cancelled) return;
						if (s) {
							found = true;
							setState(s);
							setStatus("ready");
						} else if (i === LOOKUP_DELAYS_MS.length - 1) {
							setStatus("unavailable");
						}
					})
					.catch((e) => {
						if (cancelled) return;
						setError(String(e));
						if (i === LOOKUP_DELAYS_MS.length - 1) setStatus("unavailable");
					});
			}, delay)
		);
		return () => {
			cancelled = true;
			timers.forEach(window.clearTimeout);
		};
	}, [title, active, connected, attempt]);

	const toggle = useCallback(() => {
		if (status === "setup") {
			// Settings is a separate, pre-created window; tell it which tab to open.
			emit("settings-open-tab", "notch").catch(() => {});
			invoke("open_settings_window").catch(console.error);
			return;
		}
		if (!state) {
			setAttempt((n) => n + 1);
			return;
		}
		const next = !state.liked;
		setState({ ...state, liked: next });
		invoke("spotify_set_liked", { trackId: state.track_id, liked: next }).catch((e) => {
			console.error("Spotify like failed:", e);
			setError(String(e));
			setState((s) => (s ? { ...s, liked: !next } : s));
		});
	}, [state, status]);

	const hint =
		status === "setup"
			? "Connect Spotify in Settings to save songs"
			: status === "loading"
				? "Finding this song on Spotify…"
				: status === "unavailable"
					? error || "This song isn't playing on Spotify. Click to retry"
					: state?.liked
						? "Remove from Liked Songs"
						: "Save to Liked Songs";

	return { status, liked: state?.liked ?? false, toggle, hint };
}
