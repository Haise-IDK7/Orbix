import React from "react";
import {
	Moon,
	Plane,
	Radio,
	BatteryCharging,
	Scissors,
	Lock,
	SunMedium,
	Bell,
	AppWindow,
	Gauge,
} from "lucide-react";

interface QuickActionGridProps {
	airplaneMode: boolean;
	onToggleAirplane: () => void;
	onAirplaneContextMenu?: (e: React.MouseEvent) => void;
	showAirplane?: boolean;

	hotspotActive: boolean;
	onToggleHotspot: () => void;
	onHotspotContextMenu?: (e: React.MouseEvent) => void;
	showHotspot?: boolean;

	dndActive: boolean;
	onToggleDnd: () => void;
	onDndContextMenu?: (e: React.MouseEvent) => void;
	showDnd?: boolean;

	batterySaverActive: boolean;
	onToggleBatterySaver: () => void;
	onBatterySaverContextMenu?: (e?: React.MouseEvent) => void;
	showSavings?: boolean;

	onTriggerScreenSnip: () => void;
	onLockWorkstation: () => void;
	onOpenNotificationCenter: () => void;
	onOpenNightLightSettings: () => void;
	onOpenSystemTray?: (e: React.MouseEvent) => void;

	/** Physical memory in use, 0–100; null until the first reading. */
	ramPercent: number | null;
	onOpenPerformance: () => void;
}

export const QuickActionGrid: React.FC<QuickActionGridProps> = React.memo(({
	airplaneMode,
	onToggleAirplane,
	onAirplaneContextMenu,
	showAirplane = true,

	hotspotActive,
	onToggleHotspot,
	onHotspotContextMenu,
	showHotspot = true,

	dndActive,
	onToggleDnd,
	onDndContextMenu,
	showDnd = true,

	batterySaverActive,
	onToggleBatterySaver,
	onBatterySaverContextMenu,
	showSavings = true,

	onTriggerScreenSnip,
	onLockWorkstation,
	onOpenNotificationCenter,
	onOpenNightLightSettings,
	onOpenSystemTray,
	ramPercent,
	onOpenPerformance,
}) => {
	return (
		<div className="cc-actions-grid">
			{/* Night Light */}
			<button
				type="button"
				className="cc-action-tile"
				onClick={(e) => {
					e.stopPropagation();
					onOpenNightLightSettings();
				}}
				title="Night Light • Click for Display Settings"
			>
				<SunMedium size={14} />
				<span className="cc-action-name">Night light</span>
			</button>

			{/* Airplane Mode */}
			{showAirplane && (
				<button
					type="button"
					className={`cc-action-tile ${airplaneMode ? "active" : ""}`}
					onClick={(e) => {
						e.stopPropagation();
						onToggleAirplane();
					}}
					onContextMenu={onAirplaneContextMenu}
					title={`Airplane Mode: ${airplaneMode ? "On" : "Off"} • Click to Toggle`}
				>
					<Plane size={14} />
					<span className="cc-action-name">Airplane</span>
				</button>
			)}

			{/* Mobile Hotspot */}
			{showHotspot && (
				<button
					type="button"
					className={`cc-action-tile ${hotspotActive ? "active" : ""}`}
					onClick={(e) => {
						e.stopPropagation();
						onToggleHotspot();
					}}
					onContextMenu={onHotspotContextMenu}
					title={`Mobile Hotspot: ${hotspotActive ? "On" : "Off"} • Click to Toggle`}
				>
					<Radio size={14} />
					<span className="cc-action-name">Hotspot</span>
				</button>
			)}

			{/* Focus / DND */}
			{showDnd && (
				<button
					type="button"
					className={`cc-action-tile ${dndActive ? "active" : ""}`}
					onClick={(e) => {
						e.stopPropagation();
						onToggleDnd();
					}}
					onContextMenu={onDndContextMenu}
					title={`Focus / DND: ${dndActive ? "On" : "Off"} • Click to Toggle`}
				>
					<Moon size={14} />
					<span className="cc-action-name">Focus</span>
				</button>
			)}

			{/* Energy Saver */}
			{showSavings && (
				<button
					type="button"
					className={`cc-action-tile ${batterySaverActive ? "active" : ""}`}
					onClick={(e) => {
						e.stopPropagation();
						onToggleBatterySaver();
					}}
					onContextMenu={onBatterySaverContextMenu}
					title={`Energy Saver: ${batterySaverActive ? "On" : "Off"} • Click to Toggle`}
				>
					<BatteryCharging size={14} />
					<span className="cc-action-name">Saver</span>
				</button>
			)}

			{/* Performance: RAM load at a glance, opens the Performance panel */}
			<button
				type="button"
				className={`cc-action-tile cc-perf-tile ${ramPercent !== null && ramPercent >= 85 ? "high" : ""}`}
				onClick={(e) => {
					e.stopPropagation();
					onOpenPerformance();
				}}
				title="Performance • Free up RAM and close heavy apps"
			>
				<Gauge size={14} />
				<span className="cc-action-name">{ramPercent === null ? "RAM" : `RAM ${ramPercent}%`}</span>
				{ramPercent !== null && (
					<span className="cc-perf-tile-bar" style={{ width: `${ramPercent}%` }} />
				)}
			</button>

			{/* Screen Snip */}
			<button
				type="button"
				className="cc-action-tile"
				onClick={(e) => {
					e.stopPropagation();
					onTriggerScreenSnip();
				}}
				title="Screen Snip (Win+Shift+S)"
			>
				<Scissors size={14} />
				<span className="cc-action-name">Snip</span>
			</button>

			{/* Lock Workstation */}
			<button
				type="button"
				className="cc-action-tile"
				onClick={(e) => {
					e.stopPropagation();
					onLockWorkstation();
				}}
				title="Lock PC"
			>
				<Lock size={14} />
				<span className="cc-action-name">Lock</span>
			</button>

			{/* System Tray or Notification Center */}
			{onOpenSystemTray ? (
				<button
					type="button"
					className="cc-action-tile"
					onClick={(e) => {
						e.stopPropagation();
						onOpenSystemTray(e);
					}}
					title="System Tray Icons"
				>
					<AppWindow size={14} />
					<span className="cc-action-name">Tray</span>
				</button>
			) : (
				<button
					type="button"
					className="cc-action-tile"
					onClick={(e) => {
						e.stopPropagation();
						onOpenNotificationCenter();
					}}
					title="Notifications"
				>
					<Bell size={14} />
					<span className="cc-action-name">Alerts</span>
				</button>
			)}
		</div>
	);
});
