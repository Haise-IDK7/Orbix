import React, { useState } from "react";
import { motion } from "framer-motion";
import { Minus, Plus, RotateCcw, Settings2, SkipForward, Check } from "lucide-react";
import {
	cycleRemaining,
	cycleTotal,
	phaseDuration,
	pomodoro,
	remainingNow,
	usePomodoro,
	type PomodoroConfig,
	type PomodoroState
} from "./pomodoroStore";
import "./Focus.css";

const PHASE_LABEL = { focus: "Focus", short: "Short break", long: "Long break" } as const;

export const formatClock = (ms: number) => {
	const total = Math.ceil(ms / 1000);
	const h = Math.floor(total / 3600);
	const m = Math.floor((total % 3600) / 60);
	const s = total % 60;
	const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
	return `${h > 0 ? `${h}:` : ""}${mm}:${String(s).padStart(2, "0")}`;
};

const formatPlan = (ms: number) => {
	const minutes = Math.round(ms / 60_000);
	const h = Math.floor(minutes / 60);
	const m = minutes % 60;
	return h > 0 ? `${h} h${m ? ` ${m} min` : ""}` : `${m} min`;
};

type SegmentState = "done" | "current" | "upcoming";

/** Position of each step of the cycle relative to where the timer is now. */
function segmentState(
	s: PomodoroState,
	kind: "focus" | "short" | "long",
	round: number
): SegmentState {
	const order = (phase: "focus" | "short" | "long", r: number) =>
		phase === "long" ? Number.MAX_SAFE_INTEGER : r * 2 + (phase === "short" ? 1 : 0);
	const here = order(s.phase, s.round);
	const there = order(kind, round);
	return there < here ? "done" : there === here ? "current" : "upcoming";
}

function RoundTrack({ s, progress }: { s: PomodoroState; progress: number }) {
	const rounds = Array.from({ length: s.config.sessions }, (_, i) => i + 1);
	const fill = (state: SegmentState) => (state === "done" ? 1 : state === "current" ? progress : 0);
	return (
		<div className="pomo-track">
			<div className="pomo-track-bar">
				{rounds.map((r) => {
					const focusState = segmentState(s, "focus", r);
					const breakState = segmentState(s, "short", r);
					return (
						<React.Fragment key={r}>
							<div className={`pomo-seg focus ${focusState}`}>
								<div
									className="pomo-seg-fill"
									style={{ transform: `scaleX(${fill(focusState)})` }}
								/>
								{focusState === "current" && (
									<div className="pomo-knob" style={{ left: `${progress * 100}%` }} />
								)}
							</div>
							{r < s.config.sessions && <div className={`pomo-dot ${breakState}`} />}
						</React.Fragment>
					);
				})}
				<div className={`pomo-seg long ${segmentState(s, "long", 0)}`}>
					<div
						className="pomo-seg-fill"
						style={{ transform: `scaleX(${fill(segmentState(s, "long", 0))})` }}
					/>
				</div>
			</div>
			<div className="pomo-track-labels">
				{rounds.map((r) => (
					<span key={r} className={s.round === r && s.phase !== "long" ? "current" : ""}>
						{r}
					</span>
				))}
				<span className={s.phase === "long" ? "current" : ""}>Long</span>
			</div>
		</div>
	);
}

const LIMITS: Record<keyof PomodoroConfig, [number, number]> = {
	focusMin: [5, 90],
	shortMin: [1, 30],
	longMin: [5, 60],
	sessions: [2, 8]
};
const STEP: Record<keyof PomodoroConfig, number> = {
	focusMin: 5,
	shortMin: 1,
	longMin: 5,
	sessions: 1
};
const FIELD_LABEL: Record<keyof PomodoroConfig, string> = {
	focusMin: "Focus",
	shortMin: "Break",
	longMin: "Long",
	sessions: "Rounds"
};

function DurationEditor({ config, onDone }: { config: PomodoroConfig; onDone: () => void }) {
	const [draft, setDraft] = useState(config);
	const step = (key: keyof PomodoroConfig, delta: number) => {
		const [min, max] = LIMITS[key];
		setDraft((d) => ({ ...d, [key]: Math.min(max, Math.max(min, d[key] + delta)) }));
	};
	return (
		<div className="pomo-editor">
			{(Object.keys(LIMITS) as (keyof PomodoroConfig)[]).map((key) => (
				<div key={key} className="pomo-editor-field">
					<span className="pomo-editor-label">{FIELD_LABEL[key]}</span>
					<div className="pomo-stepper">
						<button type="button" onClick={() => step(key, -STEP[key])}>
							<Minus size={11} />
						</button>
						<span>
							{draft[key]}
							{key === "sessions" ? "" : "m"}
						</span>
						<button type="button" onClick={() => step(key, STEP[key])}>
							<Plus size={11} />
						</button>
					</div>
				</div>
			))}
			<button
				type="button"
				className="pomo-pill"
				onClick={() => {
					pomodoro.configure(draft);
					onDone();
				}}
			>
				<Check size={12} /> Save
			</button>
		</div>
	);
}

/** Pomodoro panel: current session on the left, the whole cycle on the right. */
export const PomodoroView: React.FC = () => {
	const s = usePomodoro();
	const [editing, setEditing] = useState(false);
	const remaining = remainingNow(s);
	const duration = phaseDuration(s);
	const progress = duration > 0 ? 1 - remaining / duration : 0;

	return (
		<motion.div
			className="focus-panel"
			onClick={(e) => e.stopPropagation()}
			initial={{ opacity: 0, y: 6 }}
			animate={{ opacity: 1, y: 0 }}
			exit={{ opacity: 0, transition: { duration: 0.1 } }}
			transition={{ type: "spring", stiffness: 420, damping: 34 }}
		>
			<div className="panel-title-row">
				<span className="panel-title">Pomodoro</span>
				<button
					type="button"
					className="panel-icon-btn"
					onClick={() => setEditing((v) => !v)}
					title="Durations"
				>
					<Settings2 size={13} />
				</button>
			</div>
			<div className="pomo-body">
				<div className="pomo-left">
					<span className={`pomo-phase ${s.phase}`}>
						{PHASE_LABEL[s.phase]}
						{s.phase !== "long" && ` · ${s.round} of ${s.config.sessions}`}
					</span>
					<span className="pomo-clock">{formatClock(remaining)}</span>
					<span className="pomo-until">
						<b>{formatClock(cycleRemaining(s))}</b> until done
					</span>
					<div className={`pomo-progress ${s.phase}`}>
						<div style={{ transform: `scaleX(${progress})` }} />
					</div>
				</div>
				<div className="pomo-divider" />
				<div className="pomo-right">
					{editing ? (
						<DurationEditor config={s.config} onDone={() => setEditing(false)} />
					) : (
						<>
							<div className="pomo-right-head">
								<span className="pomo-muted">This round</span>
								<div className="pomo-actions">
									<button type="button" className="pomo-pill" onClick={pomodoro.toggle}>
										{s.running ? "Pause" : remaining < duration ? "Resume" : "Start"}
									</button>
									<button
										type="button"
										className="panel-icon-btn"
										onClick={pomodoro.skip}
										title="Skip"
									>
										<SkipForward size={13} />
									</button>
									<button
										type="button"
										className="panel-icon-btn"
										onClick={pomodoro.reset}
										title="Reset"
									>
										<RotateCcw size={13} />
									</button>
								</div>
							</div>
							<RoundTrack s={s} progress={Math.min(1, Math.max(0, progress))} />
							<span className="pomo-muted">
								{formatPlan(cycleTotal(s.config))} · {s.config.sessions} sessions
							</span>
						</>
					)}
				</div>
			</div>
		</motion.div>
	);
};

/** Ring + time for the collapsed notch while a session runs. */
export const PomodoroCompact: React.FC = () => {
	const s = usePomodoro();
	const remaining = remainingNow(s);
	const duration = phaseDuration(s);
	const progress = duration > 0 ? 1 - remaining / duration : 0;
	const r = 6;
	const c = 2 * Math.PI * r;
	return (
		<span className={`pomo-compact ${s.phase}`}>
			<svg width="16" height="16" viewBox="0 0 16 16" aria-hidden>
				<circle cx="8" cy="8" r={r} className="pomo-ring-bg" />
				<circle
					cx="8"
					cy="8"
					r={r}
					className="pomo-ring"
					strokeDasharray={c}
					strokeDashoffset={c * (1 - progress)}
					transform="rotate(-90 8 8)"
				/>
			</svg>
			{formatClock(remaining)}
		</span>
	);
};
