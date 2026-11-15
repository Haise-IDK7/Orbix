export type SubViewMode = "main" | "wifi" | "bluetooth" | "performance";

export interface InternetStatus {
	connection_type: "ethernet" | "wifi" | "offline";
	label: string;
	is_connected: boolean;
	ethernet_connected: boolean;
	wifi_connected: boolean;
	wifi_enabled: boolean;
	/** Connected Wi-Fi network, also while Ethernet is the main connection. */
	wifi_ssid?: string;
}

export interface WifiNetworkEntry {
	ssid: string;
	signal_percent: number;
	is_secure: boolean;
	is_connected: boolean;
	has_profile: boolean;
}

export interface MicrophoneState {
	is_muted: boolean;
	device_name: string;
}

export interface AppMemoryUsage {
	name: string;
	memory_mb: number;
	process_count: number;
}

export interface MemoryReport {
	total_mb: number;
	available_mb: number;
	used_percent: number;
	top_apps: AppMemoryUsage[];
	/** Orbix itself, including its WebView2 processes. */
	orbix_mb?: number;
}

export interface OptimizeResult {
	freed_mb: number;
	processes_trimmed: number;
	available_mb: number;
	used_percent: number;
}

export interface CloseAppResult {
	remaining: number;
}

export interface BtDevice {
	id: string;
	name: string;
	/** "audio", "input", "phone", "computer" or "other". */
	kind: string;
	connected: boolean;
	/** Orbix can connect/disconnect it (Bluetooth audio). */
	can_toggle: boolean;
	/** Battery %, when the device reports one (connected devices only). */
	battery?: number | null;
}

export interface BtOverview {
	enabled: boolean;
	devices: BtDevice[];
}
