import React from "react";
import { AnimatePresence, motion } from "framer-motion";
import "./Lyrics.css";

/** Height the collapsed notch grows by to show the current line (px). */
export const LYRICS_TICKER_HEIGHT = 24;

interface LyricsTickerProps {
	text: string;
	/** Changes on every new line, so repeated lines still animate. */
	lineKey: number;
}

/** Current lyric line under the collapsed notch, crossfading between lines. */
export const LyricsTicker: React.FC<LyricsTickerProps> = ({ text, lineKey }) => (
	<div className="lyrics-ticker" style={{ height: LYRICS_TICKER_HEIGHT }}>
		<AnimatePresence mode="wait" initial={false}>
			<motion.span
				key={lineKey}
				className="lyrics-ticker-text"
				initial={{ opacity: 0, y: 6, filter: "blur(3px)" }}
				animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
				exit={{ opacity: 0, y: -6, filter: "blur(3px)" }}
				transition={{ duration: 0.22, ease: "easeOut" }}
			>
				{text}
			</motion.span>
		</AnimatePresence>
	</div>
);
