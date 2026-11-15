export type SubViewMode = "main" | "wifi" | "bluetooth" | "performance";

export interface InternetStatus {
	connection_type: "ethernet" | "wifi" | "offline";
	label: string;
	is_connected: boolean;
	ethernet_connected: boolean;
	wifi_connected: boolean;
	wifi_enabled: boolean;
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
