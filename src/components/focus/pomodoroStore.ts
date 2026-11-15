import { useSyncExternalStore } from "react";
import { playTimerChime, getTimerChimeCtx } from "../../chime";

/**
 * Pomodoro cycle: N focus sessions separated by short breaks, then a long
 * break. Lives outside React so the ticking clock only re-renders the small
 * components that read it, never the whole notch.
 */

export type PomodoroPhase = "focus" | "short" | "long";

export interface PomodoroConfig {
	focusMin: number;
	shortMin: number;
	longMin: number;
	sessions: number;
}

export interface PomodoroState {
	phase: PomodoroPhase;
	/** 1-based focus session within the cycle. */
	round: number;
	running: boolean;
	/** Remaining time of the current phase when paused. */
	remainingMs: number;
	/** Wall-clock end of the current phase while running. */
	endsAt: number | null;
	config: PomodoroConfig;
}

const STORAGE_KEY = "bloom-pomodoro-state";
export const DEFAULT_CONFIG: PomodoroConfig = {
	focusMin: 25,
	shortMin: 5,
	longMin: 15,
	sessions: 4
};

const phaseMs = (phase: PomodoroPhase, config: PomodoroConfig) =>
	(phase === "focus" ? config.focusMin : phase === "short" ? config.shortMin : config.longMin) *
	60_000;

const fresh = (config: PomodoroConfig): PomodoroState => ({
	phase: "focus",
	round: 1,
	running: false,
	remainingMs: phaseMs("focus", config),
	endsAt: null,
	config
});

function load(): PomodoroState {
	try {
		const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) || "null");
		if (saved && typeof saved.remainingMs === "number" && saved.config) return saved;
	} catch {
		/* fall through to a fresh cycle */
	}
	return fresh(DEFAULT_CONFIG);
}

let state = load();
const listeners = new Set<() => void>();
let ticker: number | null = null;
// Bumped on every visible change (state or displayed second) — the snapshot
// subscribers compare, since ticking doesn't replace `state`.
let version = 0;
let lastShownSecond = -1;

function emit() {
	version++;
	listeners.forEach((l) => l());
}

function commit(next: PomodoroState) {
	state = next;
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
	} catch {
		/* storage full or unavailable: keep running in memory */
	}
	syncTicker();
	emit();
}

/** The phase after the current one, following the focus → break pattern. */
function nextPhase(s: PomodoroState): Pick<PomodoroState, "phase" | "round"> {
	if (s.phase === "focus") {
		return { phase: s.round >= s.config.sessions ? "long" : "short", round: s.round };
	}
	if (s.phase === "short") return { phase: "focus", round: s.round + 1 };
	return { phase: "focus", round: 1 };
}

function tick() {
	if (!state.running || state.endsAt === null) return;
	if (Date.now() < state.endsAt) {
		const second = Math.ceil((state.endsAt - Date.now()) / 1000);
		if (second !== lastShownSecond) {
			lastShownSecond = second;
			emit();
		}
		return;
	}
	playTimerChime();
	const next = nextPhase(state);
	// A finished long break completes the cycle: stop and wait for the user.
	const cycleDone = state.phase === "long";
	const duration = phaseMs(next.phase, state.config);
	commit({
		...state,
		...next,
		running: !cycleDone,
		remainingMs: duration,
		endsAt: cycleDone ? null : Date.now() + duration
	});
}

function syncTicker() {
	if (state.running && ticker === null) {
		ticker = window.setInterval(tick, 250);
	} else if (!state.running && ticker !== null) {
		clearInterval(ticker);
		ticker = null;
	}
}
syncTicker();

export const pomodoro = {
	start() {
		if (state.running) return;
		getTimerChimeCtx(); // unlock audio from a user gesture
		commit({ ...state, running: true, endsAt: Date.now() + state.remainingMs });
	},
	pause() {
		if (!state.running || state.endsAt === null) return;
		commit({
			...state,
			running: false,
			remainingMs: Math.max(0, state.endsAt - Date.now()),
			endsAt: null
		});
	},
	toggle() {
		if (state.running) pomodoro.pause();
		else pomodoro.start();
	},
	/** Ends the current phase now and moves on. */
	skip() {
		const next = nextPhase(state);
		const duration = phaseMs(next.phase, state.config);
		commit({
			...state,
			...next,
			remainingMs: duration,
			endsAt: state.running ? Date.now() + duration : null
		});
	},
	reset() {
		commit(fresh(state.config));
	},
	configure(config: PomodoroConfig) {
		const same = JSON.stringify(config) === JSON.stringify(state.config);
		if (same) return;
		// Changing durations restarts the cycle; a running session is not
		// resized under the user.
		commit(state.running ? { ...state, config } : fresh(config));
	}
};

const subscribe = (listener: () => void) => {
	listeners.add(listener);
	return () => listeners.delete(listener);
};

/** Remaining ms of the current phase right now. */
export const remainingNow = (s: PomodoroState) =>
	s.running && s.endsAt !== null ? Math.max(0, s.endsAt - Date.now()) : s.remainingMs;

/** Remaining ms of the whole cycle, through the long break. */
export function cycleRemaining(s: PomodoroState): number {
	let total = remainingNow(s);
	let cursor: Pick<PomodoroState, "phase" | "round"> = { phase: s.phase, round: s.round };
	while (cursor.phase !== "long") {
		cursor = nextPhase({ ...s, ...cursor });
		total += phaseMs(cursor.phase, s.config);
	}
	return total;
}

export const phaseDuration = (s: PomodoroState) => phaseMs(s.phase, s.config);

export const cycleTotal = (c: PomodoroConfig) =>
	(c.focusMin * c.sessions + c.shortMin * (c.sessions - 1) + c.longMin) * 60_000;

/** Whole state; re-renders on every tick while running. Use in small components. */
export function usePomodoro(): PomodoroState {
	useSyncExternalStore(
		subscribe,
		() => version,
		() => version
	);
	return state;
}

/** Only whether it is running; re-renders only when that flips. */
export function usePomodoroRunning(): boolean {
	return useSyncExternalStore(
		subscribe,
		() => state.running,
		() => state.running
	);
}
