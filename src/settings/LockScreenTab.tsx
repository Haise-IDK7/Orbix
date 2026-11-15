import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
	BatteryMedium,
	Calendar,
	CalendarDays,
	CloudSun,
	Clock,
	Eye,
	KeyRound,
	Blend,
	Image,
	Layers,
	ListChecks,
	Lock,
	Move,
	Music2,
	Palette,
	Power,
	Scaling,
	Timer,
	Type
} from "lucide-react";
import { LOCK_FONTS, lockFontById } from "../components/lockscreen/lockFonts";
import { useStoredSetting } from "../hooks/useStoredSetting";
import { SettingRow } from "./SettingRow";

function Toggle({ value, onChange }: { value: string; onChange: (v: string) => void }) {
	return (
		<label className="toggle-switch">
			<input
				type="checkbox"
				checked={value !== "false"}
				onChange={() => onChange(value === "false" ? "true" : "false")}
			/>
			<span className="slider"></span>
		</label>
	);
}

const WIDGETS = [
	{
		key: "bloom-lock-w-date",
		label: "Date",
		desc: "Weekday and date above the clock",
		icon: CalendarDays,
		def: "true"
	},
	{
		key: "bloom-lock-w-weather",
		label: "Weather",
		desc: "Temperature next to the date",
		icon: CloudSun,
		def: "true"
	},
	{
		key: "bloom-lock-w-calendar",
		label: "Calendar",
		desc: "This month at a glance",
		icon: Calendar,
		def: "true"
	},
	{
		key: "bloom-lock-w-battery",
		label: "Battery",
		desc: "Charge level (laptops)",
		icon: BatteryMedium,
		def: "true"
	},
	{
		key: "bloom-lock-w-media",
		label: "Now Playing",
		desc: "The song that is playing",
		icon: Music2,
		def: "true"
	},
	{
		key: "bloom-lock-w-tasks",
		label: "Tasks",
		desc: "Open items from the Tasks panel",
		icon: ListChecks,
		def: "false"
	}
] as const;

function WidgetToggle({ w, last }: { w: (typeof WIDGETS)[number]; last: boolean }) {
	const [value, setValue] = useStoredSetting(w.key, w.def);
	return (
		<SettingRow icon={w.icon} label={w.label} desc={w.desc} divider={!last}>
			<Toggle value={value} onChange={setValue} />
		</SettingRow>
	);
}

/** Orbix lock screen: when it appears, and how the clock and widgets look. */
export function LockScreenTab() {
	const [enabled, setEnabled] = useStoredSetting("bloom-lock-enabled", "true");
	const [idle, setIdle] = useStoredSetting("bloom-lock-idle", "5");
	const [onLock, setOnLock] = useStoredSetting("bloom-lock-on-lock", "true");
	const [wallpaper, setWallpaper] = useStoredSetting("bloom-lock-wallpaper", "");
	const [fontId, setFontId] = useStoredSetting("bloom-lock-font", "oswald");
	const [weight, setWeight] = useStoredSetting("bloom-lock-weight", "600");
	const [size, setSize] = useStoredSetting("bloom-lock-size", "100");
	const [style, setStyle] = useStoredSetting("bloom-lock-style", "depth");
	const [fusion, setFusion] = useStoredSetting(
		"bloom-lock-fusion",
		style === "solid" ? "false" : "true"
	);
	const [skipCurtain, setSkipCurtain] = useState<boolean | null>(null);
	const [curtainError, setCurtainError] = useState("");

	useEffect(() => {
		invoke<boolean>("lock_curtain_skipped")
			.then(setSkipCurtain)
			.catch(() => setSkipCurtain(false));
	}, []);

	const toggleCurtain = () => {
		const next = !skipCurtain;
		setCurtainError("");
		invoke<boolean>("set_lock_curtain_skipped", { skip: next })
			.then((state) => {
				setSkipCurtain(state);
				if (state !== next) setCurtainError("Windows didn't apply the change");
			})
			.catch((e) => setCurtainError(String(e)));
	};
	const [colorMode, setColorMode] = useStoredSetting("bloom-lock-color-mode", "auto");
	const [color, setColor] = useStoredSetting("bloom-lock-color", "#ffffff");
	const [position, setPosition] = useStoredSetting("bloom-lock-position", "top");
	const font = lockFontById(fontId);
	const on = enabled !== "false";

	const pickWallpaper = async () => {
		const selected = await open({
			multiple: false,
			title: "Lock screen wallpaper",
			filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "bmp", "webp"] }]
		}).catch(() => null);
		if (typeof selected === "string") setWallpaper(selected);
	};

	return (
		<>
			<div className="setting-group">
				<SettingRow
					icon={Lock}
					label="Orbix Lock Screen"
					desc="Shows before the Windows sign-in screen"
				>
					<Toggle value={enabled} onChange={setEnabled} />
				</SettingRow>
				{on && (
					<>
						<SettingRow
							icon={Timer}
							label="Show When Idle"
							desc="Minutes without input (skipped during video or full-screen apps)"
						>
							<select
								className="settings-select"
								value={idle}
								onChange={(e) => setIdle(e.target.value)}
							>
								<option value="0">Never</option>
								{[1, 2, 5, 10, 15, 30].map((m) => (
									<option key={m} value={String(m)}>
										{m} min
									</option>
								))}
							</select>
						</SettingRow>
						<SettingRow
							icon={Power}
							label="Use for Orbix Lock Buttons"
							desc="Lock from the notch or power menu shows it first"
						>
							<Toggle value={onLock} onChange={setOnLock} />
						</SettingRow>
						<SettingRow
							icon={KeyRound}
							label="Go Straight to PIN"
							desc={
								curtainError ||
								"Skips Windows' own lock picture after this page (whole PC, asks for admin approval)"
							}
						>
							<label className="toggle-switch">
								<input
									type="checkbox"
									checked={!!skipCurtain}
									disabled={skipCurtain === null}
									onChange={toggleCurtain}
								/>
								<span className="slider"></span>
							</label>
						</SettingRow>
						<SettingRow
							icon={Eye}
							label="Preview"
							desc="Opens it without locking the PC; press Enter or click to close"
							action
							divider={false}
							onClick={() => invoke("show_lock_screen", { preview: true }).catch(console.error)}
						/>
					</>
				)}
			</div>

			{on && (
				<>
					<div className="setting-group-label">Wallpaper</div>
					<div className="setting-group">
						<SettingRow
							icon={Image}
							label="Image"
							desc={wallpaper ? wallpaper.split(/[\\/]/).pop() : "Same as the desktop"}
							divider={false}
						>
							<div className="lock-wallpaper-actions">
								{wallpaper && (
									<button type="button" className="settings-chip" onClick={() => setWallpaper("")}>
										Use desktop
									</button>
								)}
								<button type="button" className="settings-chip" onClick={pickWallpaper}>
									Choose…
								</button>
							</div>
						</SettingRow>
					</div>

					<div className="setting-group-label">Clock</div>
					<div className="setting-group">
						<div className="font-picker">
							<div className="font-picker-title">
								<Type size={13} />
								<span>Typeface</span>
							</div>
							<div className="font-picker-grid">
								{LOCK_FONTS.map((f) => (
									<button
										type="button"
										key={f.id}
										className={`font-option ${fontId === f.id ? "active" : ""}`}
										style={{ fontFamily: f.stack }}
										onClick={() => setFontId(f.id)}
									>
										<span className="font-option-sample">9:41</span>
										<span className="font-option-name">{f.label}</span>
										<span className="font-option-desc">{f.description}</span>
									</button>
								))}
							</div>
						</div>
						<div className="setting-divider" />
						<SettingRow
							icon={Blend}
							label="Fuse With Wallpaper"
							desc="Off shows plain digits over the image"
						>
							<Toggle value={fusion} onChange={setFusion} />
						</SettingRow>
						{fusion !== "false" && (
							<SettingRow icon={Layers} label="Effect" desc="How the clock meets the wallpaper">
								<select
									className="settings-select"
									value={style === "solid" ? "depth" : style}
									onChange={(e) => setStyle(e.target.value)}
								>
									<option value="depth">Depth (subject in front)</option>
									<option value="glass">Frosted glass</option>
									<option value="blend">Blend</option>
								</select>
							</SettingRow>
						)}
						<SettingRow icon={Palette} label="Color" desc="Auto picks a tint from the wallpaper">
							<div className="lock-wallpaper-actions">
								{colorMode === "custom" && (
									<input
										type="color"
										className="lock-color-input"
										value={color}
										onChange={(e) => setColor(e.target.value)}
									/>
								)}
								<select
									className="settings-select"
									value={colorMode}
									onChange={(e) => setColorMode(e.target.value)}
								>
									<option value="auto">Auto</option>
									<option value="white">White</option>
									<option value="custom">Custom</option>
								</select>
							</div>
						</SettingRow>
						<SettingRow icon={Move} label="Position">
							<select
								className="settings-select"
								value={position}
								onChange={(e) => setPosition(e.target.value)}
							>
								<option value="top">Top</option>
								<option value="center">Center</option>
							</select>
						</SettingRow>
						<SettingRow icon={Scaling} label="Size" desc={`${size}%`}>
							<input
								type="range"
								min="60"
								max="150"
								step="5"
								value={size}
								onChange={(e) => setSize(e.target.value)}
								className="settings-slider"
							/>
						</SettingRow>
						<SettingRow
							icon={Clock}
							label="Weight"
							desc={font.variable ? weight : `${font.label} has a single weight`}
							divider={false}
						>
							<input
								type="range"
								min="200"
								max="900"
								step="50"
								value={weight}
								disabled={!font.variable}
								onChange={(e) => setWeight(e.target.value)}
								className="settings-slider"
							/>
						</SettingRow>
					</div>

					<div className="setting-group-label">Widgets</div>
					<div className="setting-group">
						{WIDGETS.map((w, i) => (
							<WidgetToggle key={w.key} w={w} last={i === WIDGETS.length - 1} />
						))}
					</div>
				</>
			)}
		</>
	);
}
