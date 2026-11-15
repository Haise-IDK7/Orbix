import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface GlassBackdrop {
	key: string;
	data_url: string;
	width: number;
	height: number;
}

const REFRESH_MS = 30_000;
const VARS = ["--glass-wallpaper", "--glass-bg-w", "--glass-bg-h", "--glass-crop-y"];

/**
 * Publishes the blurred wallpaper as CSS variables on :root, sized like the
 * desktop's default "Fill" mode (cover, centered) for this monitor, so any
 * `GlassLayer` can line it up with the real wallpaper.
 */
export function useGlassBackdrop(enabled: boolean): void {
	useEffect(() => {
		const root = document.documentElement;
		const clear = () => VARS.forEach((v) => root.style.removeProperty(v));
		if (!enabled) {
			clear();
			return;
		}

		let cancelled = false;
		let current: GlassBackdrop | null = null;

		const applySize = () => {
			if (!current) return;
			const screenW = window.screen.width;
			const screenH = window.screen.height;
			const imageRatio = current.width / current.height;
			const coverW = imageRatio > screenW / screenH ? screenH * imageRatio : screenW;
			const coverH = coverW / imageRatio;
			root.style.setProperty("--glass-bg-w", `${coverW}px`);
			root.style.setProperty("--glass-bg-h", `${coverH}px`);
			root.style.setProperty("--glass-crop-y", `${(coverH - screenH) / 2}px`);
		};

		const load = async () => {
			try {
				const backdrop = await invoke<GlassBackdrop | null>("get_glass_backdrop");
				if (cancelled) return;
				if (!backdrop) {
					current = null;
					clear();
					return;
				}
				if (backdrop.key === current?.key) return;
				current = backdrop;
				root.style.setProperty("--glass-wallpaper", `url("${backdrop.data_url}")`);
				applySize();
			} catch (err) {
				console.error("Failed to load glass backdrop:", err);
			}
		};

		void load();
		const interval = setInterval(() => void load(), REFRESH_MS);
		window.addEventListener("resize", applySize);
		return () => {
			cancelled = true;
			clearInterval(interval);
			window.removeEventListener("resize", applySize);
			clear();
		};
	}, [enabled]);
}
