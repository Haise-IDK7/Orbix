import React, { useLayoutEffect, useRef, useState } from "react";
import type { LyricsState } from "../../hooks/useLyrics";
import "./Lyrics.css";

/** Height the notch reserves for the panel below the player (px). */
export const LYRICS_PANEL_HEIGHT = 168;
/** Extra notch width for the panel beside the player (px). */
export const LYRICS_SIDE_WIDTH = 290;
/** Where the active line sits, as a fraction of the panel height. */
const FOCUS_POINT_BELOW = 0.36;
const FOCUS_POINT_BESIDE = 0.42;
/** Lines farther than this from the active one get no blur filter (they're
 * faded out by the mask anyway), keeping the number of filtered layers small. */
const MAX_BLURRED_DISTANCE = 4;

interface LyricsPanelProps {
	/** Beside the player, filling its height, instead of below it. */
	beside?: boolean;
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
	beside = false,
	state,
	activeIndex,
	canSeek,
	onSeek
}) => {
	const lineRefs = useRef<(HTMLDivElement | null)[]>([]);
	const panelRef = useRef<HTMLDivElement>(null);
	const [offset, setOffset] = useState(0);
	const variant = beside ? "beside" : "below";
	const sizeStyle = beside ? undefined : { height: LYRICS_PANEL_HEIGHT };

	// Keep the active line at the focus point; before the first line, park the
	// first line there so the song "scrolls in".
	useLayoutEffect(() => {
		if (state.status !== "synced") return;
		const el = lineRefs.current[Math.max(activeIndex, 0)];
		if (!el) return;
		const panelHeight = panelRef.current?.clientHeight || LYRICS_PANEL_HEIGHT;
		const focus = beside ? FOCUS_POINT_BESIDE : FOCUS_POINT_BELOW;
		setOffset(el.offsetTop + el.offsetHeight / 2 - panelHeight * focus);
	}, [activeIndex, state.status, state.lines, beside]);

	if (state.status === "synced") {
		return (
			<div
				ref={panelRef}
				className={`lyrics-panel synced ${variant}`}
				style={sizeStyle}
				onClick={(e) => e.stopPropagation()}
			>
				<div className="lyrics-track" style={{ transform: `translate3d(0, ${-offset}px, 0)` }}>
					{state.lines.map((line, i) => {
						const distance = Math.abs(i - activeIndex);
						const isActive = i === activeIndex;
						const blur =
							!isActive && distance <= MAX_BLURRED_DISTANCE
								? { filter: `blur(${distance * 0.45}px)` }
								: undefined;
						return (
							<div
								key={`${line.timeMs}-${i}`}
								ref={(el) => {
									lineRefs.current[i] = el;
								}}
								className={`lyrics-line ${isActive ? "active" : ""} ${i < activeIndex ? "past" : ""} ${canSeek ? "seekable" : ""}`}
								style={blur}
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
				className={`lyrics-panel plain ${variant}`}
				style={sizeStyle}
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
		<div className={`lyrics-panel status ${variant}`} style={sizeStyle}>
			<span className={state.status === "loading" ? "lyrics-status pulsing" : "lyrics-status"}>
				{STATUS_TEXT[state.status]}
			</span>
		</div>
	);
};
