import React, { useLayoutEffect, useRef, useState } from "react";
import type { LyricsState } from "../../hooks/useLyrics";
import "./Lyrics.css";

/** Height the notch reserves for the panel (px). */
export const LYRICS_PANEL_HEIGHT = 168;
/** Where the active line sits, as a fraction of the panel height. */
const FOCUS_POINT = 0.36;

interface LyricsPanelProps {
	state: LyricsState;
	activeIndex: number;
	canSeek: boolean;
	onSeek: (positionMs: number) => void;
}

const STATUS_TEXT: Partial<Record<LyricsState["status"], string>> = {
	idle: "",
	loading: "Searching lyrics…",
	notfound: "No lyrics found for this song",
	instrumental: "♪ Instrumental ♪",
	error: "Couldn't load lyrics. Check your connection."
};

export const LyricsPanel: React.FC<LyricsPanelProps> = ({
	state,
	activeIndex,
	canSeek,
	onSeek
}) => {
	const lineRefs = useRef<(HTMLDivElement | null)[]>([]);
	const [offset, setOffset] = useState(0);

	// Keep the active line at the focus point; before the first line, park the
	// first line there so the song "scrolls in".
	useLayoutEffect(() => {
		if (state.status !== "synced") return;
		const el = lineRefs.current[Math.max(activeIndex, 0)];
		if (!el) return;
		setOffset(el.offsetTop + el.offsetHeight / 2 - LYRICS_PANEL_HEIGHT * FOCUS_POINT);
	}, [activeIndex, state.status, state.lines]);

	if (state.status === "synced") {
		return (
			<div
				className="lyrics-panel synced"
				style={{ height: LYRICS_PANEL_HEIGHT }}
				onClick={(e) => e.stopPropagation()}
			>
				<div className="lyrics-track" style={{ transform: `translateY(${-offset}px)` }}>
					{state.lines.map((line, i) => {
						const distance = Math.abs(i - activeIndex);
						const isActive = i === activeIndex;
						return (
							<div
								key={`${line.timeMs}-${i}`}
								ref={(el) => {
									lineRefs.current[i] = el;
								}}
								className={`lyrics-line ${isActive ? "active" : ""} ${i < activeIndex ? "past" : ""} ${canSeek ? "seekable" : ""}`}
								style={isActive ? undefined : { filter: `blur(${Math.min(distance, 4) * 0.45}px)` }}
								onClick={() => canSeek && onSeek(line.timeMs)}
								title={canSeek ? "Jump here" : undefined}
							>
								{line.text || "♪"}
							</div>
						);
					})}
				</div>
			</div>
		);
	}

	if (state.status === "plain") {
		return (
			<div
				className="lyrics-panel plain"
				style={{ height: LYRICS_PANEL_HEIGHT }}
				onClick={(e) => e.stopPropagation()}
			>
				<div className="lyrics-plain-note">Not time-synced</div>
				{state.plainText.split(/\r?\n/).map((line, i) => (
					<div key={i} className="lyrics-plain-line">
						{line || " "}
					</div>
				))}
			</div>
		);
	}

	return (
		<div className="lyrics-panel status" style={{ height: LYRICS_PANEL_HEIGHT }}>
			<span className={state.status === "loading" ? "lyrics-status pulsing" : "lyrics-status"}>
				{STATUS_TEXT[state.status]}
			</span>
		</div>
	);
};
