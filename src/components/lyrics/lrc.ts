export interface LyricLine {
	timeMs: number;
	text: string;
}

const TIMESTAMP = /\[(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?\]/y;
const OFFSET_TAG = /^\[offset:\s*([+-]?\d+)\s*\]/i;

/**
 * Parses LRC text into time-sorted lines. Handles `[mm:ss]`, `[mm:ss.xx]`,
 * `[mm:ss.xxx]`, several timestamps on one line, metadata tags and `[offset:]`.
 * Empty lines are kept: they mark instrumental breaks.
 */
export function parseLrc(lrc: string): LyricLine[] {
	let offsetMs = 0;
	const lines: LyricLine[] = [];

	for (const rawLine of lrc.split(/\r?\n/)) {
		const raw = rawLine.trimStart();
		const offset = raw.match(OFFSET_TAG);
		if (offset) {
			offsetMs = parseInt(offset[1], 10);
			continue;
		}

		const stamps: number[] = [];
		// Sticky regex: only consecutive timestamps at the start of the line.
		TIMESTAMP.lastIndex = 0;
		let textStart = 0;
		let match: RegExpExecArray | null;
		while ((match = TIMESTAMP.exec(raw)) !== null) {
			textStart = TIMESTAMP.lastIndex;
			const minutes = parseInt(match[1], 10);
			const seconds = parseInt(match[2], 10);
			const fraction = match[3] ? parseInt(match[3].padEnd(3, "0"), 10) : 0;
			stamps.push(minutes * 60_000 + seconds * 1000 + fraction);
		}
		if (stamps.length === 0) continue; // metadata such as [ar:] or plain text

		const text = raw.slice(textStart).trim();
		for (const t of stamps) {
			// A positive LRC offset means lyrics should appear earlier.
			lines.push({ timeMs: Math.max(0, t - offsetMs), text });
		}
	}

	return lines.sort((a, b) => a.timeMs - b.timeMs);
}

/** Index of the line being sung at `positionMs`, or -1 before the first line. */
export function findActiveLine(lines: LyricLine[], positionMs: number): number {
	let lo = 0;
	let hi = lines.length - 1;
	let found = -1;
	while (lo <= hi) {
		const mid = (lo + hi) >> 1;
		if (lines[mid].timeMs <= positionMs) {
			found = mid;
			lo = mid + 1;
		} else {
			hi = mid - 1;
		}
	}
	return found;
}
