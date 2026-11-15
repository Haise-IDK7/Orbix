import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
	BatteryFull,
	BatteryLow,
	BatteryMedium,
	Bluetooth,
	ChevronLeft,
	ExternalLink,
	Headphones,
	Laptop,
	Mouse,
	Plus,
	RotateCw,
	Smartphone
} from "lucide-react";
import type { BtDevice, BtOverview } from "./types";

interface BluetoothSubViewProps {
	bluetoothEnabled: boolean;
	onToggleBluetooth: () => void;
	onBack: () => void;
	onOpenSettings: () => void;
}

/** Device list refresh while the panel is open (connections change on their own). */
const REFRESH_MS = 4000;
/** Bluetooth audio needs a moment to (dis)connect before the state flips. */
const SETTLE_MS = 2500;

const batteryIcon = (level: number) =>
	level <= 20 ? BatteryLow : level <= 60 ? BatteryMedium : BatteryFull;

const KIND_ICONS: Record<string, typeof Bluetooth> = {
	audio: Headphones,
	input: Mouse,
	phone: Smartphone,
	computer: Laptop
};

export const BluetoothSubView: React.FC<BluetoothSubViewProps> = ({
	bluetoothEnabled,
	onToggleBluetooth,
	onBack,
	onOpenSettings
}) => {
	const [devices, setDevices] = useState<BtDevice[] | null>(null);
	const [busy, setBusy] = useState<string | null>(null);
	const [error, setError] = useState<string | null>(null);

	const refresh = useCallback(() => {
		invoke<BtOverview>("bluetooth_devices")
			.then((o) => setDevices(o.devices))
			.catch(() => setDevices([]));
	}, []);

	useEffect(() => {
		if (!bluetoothEnabled) {
			setDevices(null);
			return;
		}
		refresh();
		const id = window.setInterval(refresh, REFRESH_MS);
		return () => window.clearInterval(id);
	}, [bluetoothEnabled, refresh]);

	const toggleDevice = (dev: BtDevice) => {
		setBusy(dev.id);
		setError(null);
		invoke("bluetooth_set_connected", { name: dev.name, connected: !dev.connected })
			.then(() => new Promise((r) => window.setTimeout(r, SETTLE_MS)))
			.catch((e) => setError(String(e)))
			.finally(() => {
				setBusy(null);
				refresh();
			});
	};

	const connected = devices?.filter((d) => d.connected) ?? [];
	const others = devices?.filter((d) => !d.connected) ?? [];

	const row = (dev: BtDevice) => {
		const Icon = KIND_ICONS[dev.kind] ?? Bluetooth;
		const isBusy = busy === dev.id;
		return (
			<div key={dev.id} className={`cc-bt-row ${dev.connected ? "connected" : ""}`}>
				<div className="cc-bt-row-icon">
					<Icon size={14} />
				</div>
				<div className="cc-bt-row-text">
					<span className="cc-bt-row-name">{dev.name}</span>
					<span className="cc-bt-row-status">
						{isBusy
							? dev.connected
								? "Disconnecting…"
								: "Connecting…"
							: dev.connected
								? "Connected"
								: "Paired"}
					</span>
				</div>
				{dev.connected && typeof dev.battery === "number" && (
					<span className={`cc-bt-battery ${dev.battery <= 20 ? "low" : ""}`} title="Battery">
						{React.createElement(batteryIcon(dev.battery), { size: 13 })}
						{dev.battery}%
					</span>
				)}
				{dev.can_toggle && (
					<button
						type="button"
						className={`cc-bt-row-btn ${dev.connected ? "" : "primary"}`}
						disabled={busy !== null}
						onClick={(e) => {
							e.stopPropagation();
							toggleDevice(dev);
						}}
					>
						{isBusy ? (
							<RotateCw size={11} className="spinning" />
						) : dev.connected ? (
							"Disconnect"
						) : (
							"Connect"
						)}
					</button>
				)}
			</div>
		);
	};

	return (
		<div className="cc-subview-container">
			<div className="cc-subview-header">
				<button
					type="button"
					className="cc-subview-back-btn"
					onClick={(e) => {
						e.stopPropagation();
						onBack();
					}}
					title="Back"
				>
					<ChevronLeft size={16} />
				</button>
				<span className="cc-subview-title">Bluetooth</span>
				<div className="cc-subview-header-actions">
					<button
						type="button"
						className={`cc-subview-toggle-pill ${bluetoothEnabled ? "active" : ""}`}
						onClick={(e) => {
							e.stopPropagation();
							onToggleBluetooth();
						}}
						title={bluetoothEnabled ? "Turn Bluetooth Off" : "Turn Bluetooth On"}
					>
						{bluetoothEnabled ? "On" : "Off"}
					</button>
				</div>
			</div>

			<div className="cc-subview-body">
				{!bluetoothEnabled ? (
					<div className="cc-subview-empty">
						<p>Bluetooth is off</p>
						<button
							type="button"
							className="cc-subview-primary-btn"
							onClick={(e) => {
								e.stopPropagation();
								onToggleBluetooth();
							}}
						>
							Turn On
						</button>
					</div>
				) : devices === null ? (
					<div className="cc-subview-loading">
						<RotateCw size={18} className="spinning" />
						<span>Looking for your devices…</span>
					</div>
				) : (
					<div className="cc-bt-list">
						{error && <p className="cc-bt-error">{error}</p>}
						{connected.length > 0 && <span className="cc-bt-section">Connected</span>}
						{connected.map(row)}
						{others.length > 0 && <span className="cc-bt-section">My devices</span>}
						{others.map(row)}
						{devices.length === 0 && <p className="cc-bt-empty">No paired devices yet.</p>}
						<div className="cc-bt-actions">
							<button
								type="button"
								className="cc-bt-action"
								onClick={(e) => {
									e.stopPropagation();
									invoke("bluetooth_add_device").catch(console.error);
								}}
							>
								<Plus size={13} />
								<span>Add device</span>
							</button>
							<button
								type="button"
								className="cc-bt-action"
								onClick={(e) => {
									e.stopPropagation();
									onOpenSettings();
								}}
							>
								<ExternalLink size={12} />
								<span>Settings</span>
							</button>
						</div>
					</div>
				)}
			</div>
		</div>
	);
};
