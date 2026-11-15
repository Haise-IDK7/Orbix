import React, { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { BatteryCharging, BatteryMedium, Check, CloudSun, Music2 } from "lucide-react";
import { useStoredSetting } from "../../hooks/useStoredSetting";
import { lockFontById } from "./lockFonts";
import { blurredCopy, clockPalette, type ClockPalette } from "./palette";
import "./LockScreen.css";

interface LockWallpaper {
	key: string;
	data_url: string;
	width: number;
	height: number;
}

interface MediaInfo {
	title: string;
	artist: string;
	is_playing: boolean;
	has_media: boolean;
	artwork?: string[];
}

/** Input this soon after showing is the click/keypress that opened it. */
const GRACE_MS = 900;
/** Clicks on these don't unlock. */
const WIDGET_SELECTOR = ".ls-widget, .ls-media";

const useBool = (key: string, fallback: boolean) => {
	const [value] = useStoredSetting(key, String(fallback));
	return value !== "false";
};

function useNow(intervalMs = 1000) {
	const [now, setNow] = useState(() => new Date());
	useEffect(() => {
		const id = window.setInterval(() => setNow(new Date()), intervalMs);
		return () => window.clearInterval(id);
	}, [intervalMs]);
	return now;
}

function MiniCalendar({ now }: { now: Date }) {
	const year = now.getFullYear();
	const month = now.getMonth();
	const first = (new Date(year, month, 1).getDay() + 6) % 7; // Monday first
	const days = new Date(year, month + 1, 0).getDate();
	const labels = Array.from({ length: 7 }, (_, i) =>
		new Intl.DateTimeFormat(undefined, { weekday: "narrow" }).format(new Date(2024, 0, 1 + i))
	);
	const cells = [...Array(first).fill(null), ...Array.from({ length: days }, (_, i) => i + 1)];
	return (
		<div className="ls-widget ls-calendar">
			<span className="ls-widget-title">
				{new Intl.DateTimeFormat(undefined, { month: "long" }).format(now)}
			</span>
			<div className="ls-cal-grid">
				{labels.map((l, i) => (
					<span key={`l${i}`} className="ls-cal-label">
						{l}
					</span>
				))}
				{cells.map((d, i) => (
					<span key={i} className={d === now.getDate() ? "today" : ""}>
						{d ?? ""}
					</span>
				))}
			</div>
		</div>
	);
}

function readTasks(): { text: string; done: boolean }[] {
	try {
		const list = JSON.parse(localStorage.getItem("bloom-tasks") || "[]");
		return Array.isArray(list) ? list : [];
	} catch {
		return [];
	}
}

/**
 * Full-screen lock page: wallpaper, a large tinted clock (solid, glass, blend
 * or with the wallpaper's subject in front of it), and optional widgets.
 */
export const LockScreen: React.FC = () => {
	const [fontId] = useStoredSetting("bloom-lock-font", "oswald");
	const [weight] = useStoredSetting("bloom-lock-weight", "600");
	const [size] = useStoredSetting("bloom-lock-size", "100");
	const [storedStyle] = useStoredSetting("bloom-lock-style", "depth");
	// "Fuse with wallpaper" off = plain digits. Older installs stored "solid"
	// as the style itself, so that still means off.
	const [fusion] = useStoredSetting(
		"bloom-lock-fusion",
		storedStyle === "solid" ? "false" : "true"
	);
	const style = fusion === "false" ? "solid" : storedStyle === "solid" ? "depth" : storedStyle;
	const [colorMode] = useStoredSetting("bloom-lock-color-mode", "auto");
	const [customColor] = useStoredSetting("bloom-lock-color", "#ffffff");
	const [position] = useStoredSetting("bloom-lock-position", "top");
	const [wallpaperPath] = useStoredSetting("bloom-lock-wallpaper", "");
	const [timeFormat24] = useStoredSetting("bloom-time-format-24h", "false");
	const showDate = useBool("bloom-lock-w-date", true);
	const showWeather = useBool("bloom-lock-w-weather", true);
	const showBattery = useBool("bloom-lock-w-battery", true);
	const showMedia = useBool("bloom-lock-w-media", true);
	const showCalendar = useBool("bloom-lock-w-calendar", true);
	const showTasks = useBool("bloom-lock-w-tasks", false);

	const [visible, setVisible] = useState(false);
	const [wallpaper, setWallpaper] = useState<LockWallpaper | null>(null);
	const [palette, setPalette] = useState<ClockPalette | null>(null);
	const [blurUrl, setBlurUrl] = useState<string | null>(null);
	const [subjectReady, setSubjectReady] = useState(false);
	const [media, setMedia] = useState<MediaInfo | null>(null);
	const [battery, setBattery] = useState<{ level: number; charging: boolean } | null>(null);
	const [tasks, setTasks] = useState(readTasks);
	const subjectHost = useRef<HTMLDivElement>(null);
	const subjectCache = useRef<{ key: string; canvas: HTMLCanvasElement } | null>(null);
	const shownAt = useRef(0);
	const now = useNow();

	// Media keeps flowing to this (hidden) window, so the card is current.
	useEffect(() => {
		const unlisten = listen<MediaInfo>("media-update", (e) => setMedia(e.payload));
		return () => {
			unlisten.then((f) => f());
		};
	}, []);

	const loadWallpaper = useCallback(() => {
		invoke<LockWallpaper | null>("get_lock_wallpaper", { path: wallpaperPath || null })
			.then((w) => setWallpaper((prev) => (prev?.key === w?.key ? prev : w)))
			.catch(console.error);
	}, [wallpaperPath]);

	useEffect(() => {
		loadWallpaper();
	}, [loadWallpaper]);

	useEffect(() => {
		const unlisten = listen<boolean>("lockscreen-show", () => {
			shownAt.current = Date.now();
			setTasks(readTasks());
			loadWallpaper();
			(navigator as any)
				.getBattery?.()
				.then((b: any) => setBattery({ level: Math.round(b.level * 100), charging: b.charging }))
				.catch(() => {});
			setVisible(true);
		});
		return () => {
			unlisten.then((f) => f());
		};
	}, [loadWallpaper]);

	// Colors, frosted copy and (for the depth style) the subject cut-out are
	// derived once per wallpaper while the window is still hidden.
	useEffect(() => {
		if (!wallpaper) return;
		let cancelled = false;
		const img = new Image();
		img.src = wallpaper.data_url;
		img
			.decode()
			.then(async () => {
				if (cancelled) return;
				setPalette(clockPalette(img, position === "center" ? "center" : "top"));
				setBlurUrl(blurredCopy(img));
				if (style !== "depth") return;
				if (subjectCache.current?.key !== wallpaper.key) {
					setSubjectReady(false);
					const { subjectLayer } = await import("./depthLayer");
					const canvas = await subjectLayer(img, window.screen.width, window.screen.height);
					if (cancelled) return;
					subjectCache.current = { key: wallpaper.key, canvas };
				}
				setSubjectReady(true);
			})
			.catch((e) => console.error("Lock screen wallpaper:", e));
		return () => {
			cancelled = true;
		};
	}, [wallpaper, style, position]);

	useEffect(() => {
		const host = subjectHost.current;
		if (!host) return;
		host.replaceChildren();
		if (style === "depth" && subjectReady && subjectCache.current) {
			host.appendChild(subjectCache.current.canvas);
		}
	}, [style, subjectReady, visible]);

	const dismiss = useCallback(() => {
		if (!visible || Date.now() - shownAt.current < GRACE_MS) return;
		setVisible(false);
		invoke("dismiss_lock_screen").catch(console.error);
	}, [visible]);

	useEffect(() => {
		if (!visible) return;
		const onKey = (e: KeyboardEvent) => {
			if (e.key === "Enter") dismiss();
		};
		const onBlur = () => window.setTimeout(dismiss, 50);
		window.addEventListener("keydown", onKey);
		window.addEventListener("blur", onBlur);
		return () => {
			window.removeEventListener("keydown", onKey);
			window.removeEventListener("blur", onBlur);
		};
	}, [visible, dismiss]);

	const font = lockFontById(fontId);
	const clockColor =
		colorMode === "white"
			? "rgba(255,255,255,0.95)"
			: colorMode === "custom"
				? customColor
				: (palette?.clock ?? "#fff");
	const textColor =
		colorMode === "auto" && palette && !palette.darkBehindClock
			? palette.accent
			: "rgba(255,255,255,0.92)";
	const hours = now.getHours();
	const h = timeFormat24 === "true" ? String(hours).padStart(2, "0") : String(hours % 12 || 12);
	const time = `${h}:${String(now.getMinutes()).padStart(2, "0")}`;
	const dateLine = new Intl.DateTimeFormat(undefined, {
		weekday: "long",
		day: "numeric",
		month: "long"
	}).format(now);
	const temp = localStorage.getItem("bloom-weather-cached-temp");
	const condition = localStorage.getItem("bloom-weather-cached-condition");
	const unit = localStorage.getItem("bloom-temp-unit") === "fahrenheit" ? "°F" : "°C";
	const openTasks = tasks.filter((t) => !t.done);
	const scale = Math.min(150, Math.max(60, parseInt(size, 10) || 100)) / 100;

	return (
		<div
			className={`ls-root ${visible ? "visible" : ""}`}
			onPointerDown={(e) => {
				if (e.button === 0 && !(e.target as Element).closest(WIDGET_SELECTOR)) dismiss();
			}}
			style={{ "--ls-blur": blurUrl ? `url("${blurUrl}")` : "none" } as React.CSSProperties}
		>
			{wallpaper && <img className="ls-bg" src={wallpaper.data_url} alt="" draggable={false} />}
			<div className="ls-shade" />

			{/* Not a motion/opacity wrapper: that would isolate the "blend" style
			    from the wallpaper it has to mix with. */}
			{visible && (
				<div className={`ls-content pos-${position}`}>
					{showDate && (
						<motion.div
							className="ls-date"
							style={{ color: textColor }}
							initial={{ y: -10, opacity: 0 }}
							animate={{ y: 0, opacity: 1 }}
							transition={{ delay: 0.1, type: "spring", stiffness: 200, damping: 24 }}
						>
							{dateLine}
							{showWeather && temp && (
								<span className="ls-weather">
									<CloudSun size={16} /> {Math.round(Number(temp))}
									{unit} {condition ? `· ${condition}` : ""}
								</span>
							)}
						</motion.div>
					)}
					<motion.div
						className={`ls-clock style-${style}`}
						style={{
							fontFamily: font.stack,
							fontWeight: font.variable ? Number(weight) : undefined,
							color: style === "glass" ? undefined : clockColor,
							fontSize: `calc(min(26vw, 46vh) * ${scale})`
						}}
						initial={{ scale: 1.06, opacity: 0 }}
						animate={{ scale: 1, opacity: 1 }}
						transition={{ type: "spring", stiffness: 120, damping: 20 }}
					>
						{time}
					</motion.div>
				</div>
			)}

			<div ref={subjectHost} className={`ls-subject ${visible && subjectReady ? "shown" : ""}`} />

			<AnimatePresence>
				{visible && (
					<motion.div
						key="widgets"
						className="ls-widgets"
						initial={{ opacity: 0, x: -16 }}
						animate={{ opacity: 1, x: 0 }}
						exit={{ opacity: 0 }}
						transition={{ delay: 0.2, type: "spring", stiffness: 160, damping: 22 }}
						style={{ color: textColor }}
					>
						{showCalendar && <MiniCalendar now={now} />}
						{showBattery && battery && (
							<div className="ls-widget ls-battery">
								{battery.charging ? <BatteryCharging size={18} /> : <BatteryMedium size={18} />}
								<span>{battery.level}%</span>
								<div className="ls-battery-bar">
									<div style={{ width: `${battery.level}%` }} />
								</div>
							</div>
						)}
						{showTasks && openTasks.length > 0 && (
							<div className="ls-widget ls-tasks">
								<span className="ls-widget-title">Tasks</span>
								{openTasks.slice(0, 4).map((t, i) => (
									<span key={i} className="ls-task">
										<span className="ls-task-dot" /> {t.text}
									</span>
								))}
								{tasks.length - openTasks.length > 0 && (
									<span className="ls-task done">
										<Check size={12} /> {tasks.length - openTasks.length} done
									</span>
								)}
							</div>
						)}
					</motion.div>
				)}
			</AnimatePresence>

			<AnimatePresence>
				{visible && showMedia && media?.has_media && (
					<motion.div
						key="media"
						className="ls-media"
						initial={{ opacity: 0, y: 20 }}
						animate={{ opacity: 1, y: 0 }}
						exit={{ opacity: 0 }}
						transition={{ delay: 0.25, type: "spring", stiffness: 160, damping: 22 }}
					>
						{media.artwork?.[0] ? (
							<img src={media.artwork[0]} alt="" />
						) : (
							<div className="ls-media-art">
								<Music2 size={20} />
							</div>
						)}
						<div className="ls-media-text">
							<span className="ls-media-title">{media.title}</span>
							<span className="ls-media-artist">{media.artist}</span>
						</div>
						<span className={`ls-media-state ${media.is_playing ? "playing" : ""}`}>
							<i />
							<i />
							<i />
						</span>
					</motion.div>
				)}
			</AnimatePresence>

			{visible && <div className="ls-hint">Press Enter or click to unlock</div>}
		</div>
	);
};
