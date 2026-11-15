import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, ChevronUp, Pin, Plus, Search, X } from "lucide-react";
import type { AppInfo } from "../dockApps";
import { useStoredSetting } from "../hooks/useStoredSetting";

const MAX_CHOICES = [0, 3, 4, 5, 6, 8, 10, 12];

const sameEntry = (a: AppInfo, b: AppInfo) => a.path.toLowerCase() === b.path.toLowerCase();

/**
 * Choose which apps live in the dock's fixed section, their order, and how many
 * of them are shown. Edits are saved immediately and pushed to the dock.
 */
export function PinnedAppsManager() {
	const [maxPinnedValue, setMaxPinned] = useStoredSetting("bloom-dock-max-pinned", "0");
	const maxPinned = parseInt(maxPinnedValue, 10) || 0;
	// Bumped after every edit so the dock window reloads its pinned list.
	const [, setRevision] = useStoredSetting("bloom-dock-pinned-revision", "0");
	const [pinned, setPinned] = useState<AppInfo[]>([]);
	const [icons, setIcons] = useState<Record<string, string>>({});
	const [installed, setInstalled] = useState<AppInfo[] | null>(null);
	const [adding, setAdding] = useState(false);
	const [query, setQuery] = useState("");

	useEffect(() => {
		invoke<AppInfo[]>("load_pinned_apps").then(setPinned).catch(console.error);
	}, []);

	// Icons load lazily and are shared between the pinned list and the picker.
	const loadIcon = (app: AppInfo) => {
		if (icons[app.path]) return;
		invoke<string | null>("get_app_icon", { path: app.path, name: app.name, hwnd: null })
			.then((icon) => icon && setIcons((prev) => ({ ...prev, [app.path]: icon })))
			.catch(() => {});
	};
	useEffect(() => {
		pinned.forEach(loadIcon);
	}, [pinned]);

	const save = (next: AppInfo[]) => {
		setPinned(next);
		invoke("save_pinned_apps", { apps: next })
			.then(() => setRevision(String(Date.now())))
			.catch(console.error);
	};

	const move = (index: number, delta: number) => {
		const target = index + delta;
		if (target < 0 || target >= pinned.length) return;
		const next = [...pinned];
		[next[index], next[target]] = [next[target], next[index]];
		save(next);
	};

	const openPicker = () => {
		setAdding(true);
		if (installed === null) {
			invoke<AppInfo[]>("get_installed_apps")
				.then((apps) => setInstalled([...apps].sort((a, b) => a.name.localeCompare(b.name))))
				.catch(() => setInstalled([]));
		}
	};

	const candidates = useMemo(() => {
		const q = query.trim().toLowerCase();
		return (installed ?? [])
			.filter((app) => !pinned.some((p) => sameEntry(p, app)))
			.filter((app) => !q || app.name.toLowerCase().includes(q))
			.slice(0, 40);
	}, [installed, pinned, query]);

	useEffect(() => {
		candidates.slice(0, 12).forEach(loadIcon);
	}, [candidates]);

	const iconFor = (app: AppInfo) =>
		icons[app.path] ? (
			<img src={icons[app.path]} alt="" className="pinned-app-icon" draggable={false} />
		) : (
			<span className="pinned-app-icon fallback">{app.name[0]}</span>
		);

	return (
		<div className="pinned-manager">
			<div className="pinned-manager-head">
				<div className="pinned-manager-title">
					<Pin size={13} />
					<span>Pinned apps</span>
				</div>
				<label className="pinned-manager-max">
					Show
					<select
						value={maxPinned}
						onChange={(e) => setMaxPinned(e.target.value)}
						className="settings-select"
					>
						{MAX_CHOICES.map((n) => (
							<option key={n} value={n}>
								{n === 0 ? "All" : `First ${n}`}
							</option>
						))}
					</select>
				</label>
			</div>

			<div className="pinned-list">
				{pinned.length === 0 && <div className="pinned-empty">No pinned apps yet.</div>}
				{pinned.map((app, i) => {
					const hidden = maxPinned > 0 && i >= maxPinned;
					return (
						<div key={app.path} className={`pinned-row ${hidden ? "hidden-by-limit" : ""}`}>
							{iconFor(app)}
							<span className="pinned-name">{app.name}</span>
							{hidden && <span className="pinned-badge">Hidden</span>}
							<button
								type="button"
								className="pinned-btn"
								onClick={() => move(i, -1)}
								disabled={i === 0}
								title="Move left"
							>
								<ChevronUp size={13} />
							</button>
							<button
								type="button"
								className="pinned-btn"
								onClick={() => move(i, 1)}
								disabled={i === pinned.length - 1}
								title="Move right"
							>
								<ChevronDown size={13} />
							</button>
							<button
								type="button"
								className="pinned-btn danger"
								onClick={() => save(pinned.filter((_, j) => j !== i))}
								title="Unpin"
							>
								<X size={13} />
							</button>
						</div>
					);
				})}
			</div>

			{adding ? (
				<div className="pinned-picker">
					<div className="pinned-search">
						<Search size={13} />
						<input
							autoFocus
							value={query}
							onChange={(e) => setQuery(e.target.value)}
							placeholder="Search installed apps"
						/>
						<button
							type="button"
							className="pinned-btn"
							onClick={() => setAdding(false)}
							title="Close"
						>
							<X size={13} />
						</button>
					</div>
					<div className="pinned-picker-list">
						{installed === null && <div className="pinned-empty">Loading apps…</div>}
						{installed !== null && candidates.length === 0 && (
							<div className="pinned-empty">No matching apps.</div>
						)}
						{candidates.map((app) => (
							<button
								type="button"
								key={app.path}
								className="pinned-row pickable"
								onMouseEnter={() => loadIcon(app)}
								onClick={() =>
									save([...pinned, { ...app, is_pinned: true, is_running: false, hwnd: undefined }])
								}
							>
								{iconFor(app)}
								<span className="pinned-name">{app.name}</span>
								<Plus size={13} className="pinned-add-icon" />
							</button>
						))}
					</div>
				</div>
			) : (
				<button type="button" className="pinned-add" onClick={openPicker}>
					<Plus size={13} /> Add app
				</button>
			)}
		</div>
	);
}
