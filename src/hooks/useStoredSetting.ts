import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsSync } from "./useSettingsSync";

/**
 * A string setting stored like every other one (localStorage + settings.json
 * via `save_setting`) and kept in sync across the notch, dock and settings
 * windows. Lets a component own its setting without threading props through
 * useSettings.
 */
export function useStoredSetting(key: string, fallback: string): [string, (value: string) => void] {
	const [value, setValue] = useState(() => localStorage.getItem(key) ?? fallback);
	useSettingsSync({ [key]: (next: unknown) => setValue(String(next)) });
	const update = useCallback(
		(next: string) => {
			setValue(next);
			localStorage.setItem(key, next);
			invoke("save_setting", { key, value: next }).catch(console.error);
		},
		[key]
	);
	return [value, update];
}
