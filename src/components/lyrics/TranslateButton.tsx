import React from "react";
import { Languages } from "lucide-react";
import type { TranslationState } from "./useLyricsTranslation";

const LANGUAGE_NAMES: Record<string, string> = {
	es: "Spanish",
	en: "English",
	pt: "Portuguese",
	fr: "French",
	de: "German",
	it: "Italian"
};

const ENGINE_NAMES: Record<string, string> = {
	device: "translated on this PC",
	microsoft: "Microsoft Translator",
	mymemory: "MyMemory"
};

/** Translate toggle beside the lyrics button; its tooltip carries the status. */
export const TranslateButton: React.FC<{
	on: boolean;
	onToggle: () => void;
	state: TranslationState;
	target: string;
}> = ({ on, onToggle, state, target }) => {
	const name = LANGUAGE_NAMES[target] ?? target.toUpperCase();
	const title =
		state.status === "error"
			? state.message
			: state.status === "same"
				? `Already in ${name}`
				: on
					? `Hide translation${state.status === "ready" ? ` (${ENGINE_NAMES[state.engine]})` : ""}`
					: `Translate to ${name}`;
	return (
		<button
			type="button"
			className={`lyrics-translate-btn ${on ? "active" : ""} ${state.status === "loading" ? "loading" : ""} ${state.status === "error" ? "error" : ""}`}
			onClick={(e) => {
				e.stopPropagation();
				onToggle();
			}}
			title={title}
			aria-label={title}
		>
			<Languages size={13} />
			{on && <span>{state.status === "loading" ? "…" : target.toUpperCase()}</span>}
		</button>
	);
};
