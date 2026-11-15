import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Copy, Heart } from "lucide-react";
import { useStoredSetting } from "../hooks/useStoredSetting";

const REDIRECT_URI = "http://127.0.0.1:43821/callback";

/**
 * Connects a Spotify account so the notch can like songs. Spotify requires a
 * developer app per user; this walks through it in three short steps.
 */
export function SpotifySettings() {
	const [clientId, setClientId] = useStoredSetting("bloom-spotify-client-id", "");
	const [connected, setConnected] = useStoredSetting("bloom-spotify-connected", "false");
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const [copied, setCopied] = useState(false);

	// The token file is the source of truth (it may have been removed).
	useEffect(() => {
		invoke<boolean>("spotify_status")
			.then((ok) => setConnected(ok ? "true" : "false"))
			.catch(() => {});
	}, [setConnected]);

	const connect = async () => {
		setBusy(true);
		setError(null);
		try {
			await invoke("spotify_connect", { clientId });
			setConnected("true");
		} catch (e) {
			setError(String(e));
		} finally {
			setBusy(false);
		}
	};

	const disconnect = async () => {
		await invoke("spotify_disconnect").catch(() => {});
		setConnected("false");
	};

	return (
		<div className="spotify-settings">
			<div className="spotify-head">
				<Heart size={14} />
				<span>Like songs on Spotify</span>
				<span className={`spotify-badge ${connected === "true" ? "on" : ""}`}>
					{connected === "true" ? "Connected" : "Not connected"}
				</span>
			</div>

			{connected === "true" ? (
				<div className="spotify-row">
					<span className="spotify-note">
						A heart appears in the music player while Spotify is playing.
					</span>
					<button type="button" className="spotify-btn ghost" onClick={disconnect}>
						Disconnect
					</button>
				</div>
			) : (
				<>
					<ol className="spotify-steps">
						<li>
							Open the{" "}
							<a
								href="#"
								onClick={(e) => {
									e.preventDefault();
									openUrl("https://developer.spotify.com/dashboard").catch(() => {});
								}}
							>
								Spotify Developer Dashboard
							</a>{" "}
							and create an app (any name; select "Web API").
						</li>
						<li>
							Add this Redirect URI to the app:
							<button
								type="button"
								className="spotify-copy"
								onClick={() => {
									navigator.clipboard.writeText(REDIRECT_URI).then(() => {
										setCopied(true);
										setTimeout(() => setCopied(false), 1500);
									});
								}}
							>
								<code>{REDIRECT_URI}</code> <Copy size={11} /> {copied ? "Copied" : ""}
							</button>
						</li>
						<li>
							Paste the app's Client ID here and connect. The app owner needs Spotify Premium.
						</li>
					</ol>
					<div className="spotify-row">
						<input
							className="spotify-input"
							placeholder="Client ID"
							value={clientId}
							onChange={(e) => setClientId(e.target.value.trim())}
							spellCheck={false}
						/>
						<button
							type="button"
							className="spotify-btn"
							disabled={busy || !clientId}
							onClick={connect}
						>
							{busy ? "Waiting for browser…" : "Connect"}
						</button>
					</div>
				</>
			)}
			{error && <div className="spotify-error">{error}</div>}
		</div>
	);
}
