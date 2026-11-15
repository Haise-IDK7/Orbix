import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Languages } from "lucide-react";
import { onDeviceTranslatorPresent } from "../components/lyrics/onDeviceTranslate";
import { useStoredSetting } from "../hooks/useStoredSetting";

/**
 * Which engines translate lyrics: Microsoft Translator with the user's free
 * key (2M characters/month) if saved, else MyMemory (~5,000/day, the
 * default). The translator built into WebView2 is an opt-in experiment.
 */
export function TranslationSettings() {
	const [email, setEmail] = useStoredSetting("bloom-translate-email", "");
	const [savedRegion, setSavedRegion] = useState<string | null | undefined>(undefined);
	const [key, setKey] = useState("");
	const [region, setRegion] = useState("");
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState("");
	const onDevice = onDeviceTranslatorPresent();
	const [useOnDevice, setUseOnDevice] = useStoredSetting("bloom-translate-on-device", "false");
	const deviceOn = onDevice && useOnDevice === "true";

	useEffect(() => {
		invoke<string | null>("translator_key_status")
			.then(setSavedRegion)
			.catch(() => setSavedRegion(null));
	}, []);

	const save = () => {
		setBusy(true);
		setError("");
		invoke("set_translator_key", { key, region: region || "global" })
			.then(() => {
				setSavedRegion(region || "global");
				setKey("");
			})
			.catch((e) => setError(String(e)))
			.finally(() => setBusy(false));
	};

	const remove = () => {
		invoke("clear_translator_key").catch(() => {});
		setSavedRegion(null);
	};

	return (
		<div className="spotify-settings">
			<div className="spotify-head">
				<Languages size={14} />
				<span>Lyrics translation engines</span>
			</div>

			<div className="translation-engine">
				<span className={`spotify-badge ${deviceOn ? "on" : ""}`}>
					{!onDevice ? "Not here" : deviceOn ? "On" : "Off"}
				</span>
				<div>
					<strong>1. On this PC (experimental, no limit)</strong>
					<p>
						{onDevice
							? "Uses the translator built into Microsoft Edge/WebView2. Off by default: on some PCs Windows offers it but never finishes loading its model. If it doesn't answer, Orbix switches to the engines below for the rest of the session."
							: "Needs a newer WebView2 (version 148 or later). Until then the engines below are used."}
					</p>
					{onDevice && (
						<label className="toggle-switch">
							<input
								type="checkbox"
								checked={useOnDevice === "true"}
								onChange={() => setUseOnDevice(useOnDevice === "true" ? "false" : "true")}
							/>
							<span className="slider"></span>
						</label>
					)}
				</div>
			</div>

			<div className="translation-engine">
				<span className={`spotify-badge ${savedRegion ? "on" : ""}`}>
					{savedRegion ? "Saved" : "Optional"}
				</span>
				<div>
					<strong>2. Microsoft Translator (2 million characters a month, free)</strong>
					<p>
						In the{" "}
						<a
							href="#"
							onClick={(e) => {
								e.preventDefault();
								openUrl(
									"https://portal.azure.com/#create/Microsoft.CognitiveServicesTextTranslation"
								).catch(() => {});
							}}
						>
							Azure portal
						</a>
						, create a Translator resource with the <b>Free F0</b> tier, then copy Key 1 and its
						Location/Region from "Keys and Endpoint". About 1,000 songs a month; no daily cap. The
						key is stored encrypted on this PC.
					</p>
					{savedRegion ? (
						<div className="spotify-row">
							<span className="spotify-note">Key saved ({savedRegion}).</span>
							<button type="button" className="spotify-btn ghost" onClick={remove}>
								Remove
							</button>
						</div>
					) : (
						<div className="spotify-row">
							<input
								className="spotify-input"
								type="password"
								placeholder="Key 1"
								value={key}
								onChange={(e) => setKey(e.target.value.trim())}
								spellCheck={false}
							/>
							<input
								className="spotify-input translation-region"
								placeholder="eastus"
								value={region}
								onChange={(e) => setRegion(e.target.value.trim())}
								spellCheck={false}
							/>
							<button type="button" className="spotify-btn" disabled={busy || !key} onClick={save}>
								{busy ? "Checking…" : "Save"}
							</button>
						</div>
					)}
					{error && <div className="spotify-error">{error}</div>}
				</div>
			</div>

			<div className="translation-engine">
				<span className="spotify-badge on">Default</span>
				<div>
					<strong>3. MyMemory (default)</strong>
					<p>
						No sign-up, about 5,000 characters a day (2–3 songs). An email raises it to 50,000; it's
						sent only to MyMemory.
					</p>
					<input
						className="spotify-input"
						type="email"
						placeholder="you@example.com (optional)"
						value={email}
						onChange={(e) => setEmail(e.target.value.trim())}
						spellCheck={false}
					/>
				</div>
			</div>
		</div>
	);
}
