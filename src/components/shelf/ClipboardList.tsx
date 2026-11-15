import React, { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "framer-motion";
import { Check, ClipboardList as ClipboardIcon, Files, Pin, X } from "lucide-react";
import "./Clipboard.css";

export interface ClipSummary {
	id: number;
	kind: "text" | "image" | "files";
	preview: string;
	thumbnail: string | null;
	char_count: number;
	file_count: number;
	timestamp: number;
}

export interface ShelfPaste {
	paths: string[];
	text: string | null;
}

const timeAgo = (ts: number, now: number) => {
	const s = Math.max(0, Math.round((now - ts) / 1000));
	if (s < 45) return "Just now";
	const m = Math.round(s / 60);
	if (m < 60) return `${m} min ago`;
	const h = Math.round(m / 60);
	if (h < 24) return `${h} h ago`;
	return `${Math.round(h / 24)} d ago`;
};

interface ClipboardListProps {
	/** Pins an entry into the Files tab of the shelf. */
	onPinToShelf: (paste: ShelfPaste) => void;
	onWheel?: (e: React.WheelEvent<HTMLDivElement>) => void;
}

/** Recent clipboard entries; click one to copy it again. */
export const ClipboardList: React.FC<ClipboardListProps> = ({ onPinToShelf, onWheel }) => {
	const [items, setItems] = useState<ClipSummary[]>([]);
	const [copiedId, setCopiedId] = useState<number | null>(null);
	const [now, setNow] = useState(() => Date.now());
	const copiedTimer = useRef<number | null>(null);

	useEffect(() => {
		invoke<ClipSummary[]>("get_clipboard_history")
			.then(setItems)
			.catch(() => {});
		const unlisten = listen<ClipSummary[]>("clipboard-history-changed", (e) => setItems(e.payload));
		// Relative times only need minute precision.
		const clock = window.setInterval(() => setNow(Date.now()), 30_000);
		return () => {
			unlisten.then((f) => f());
			window.clearInterval(clock);
			if (copiedTimer.current) window.clearTimeout(copiedTimer.current);
		};
	}, []);

	const copy = (item: ClipSummary) => {
		invoke("clipboard_restore", { id: item.id })
			.then(() => {
				setCopiedId(item.id);
				if (copiedTimer.current) window.clearTimeout(copiedTimer.current);
				copiedTimer.current = window.setTimeout(() => setCopiedId(null), 1400);
			})
			.catch(console.error);
	};

	if (items.length === 0) {
		return (
			<div className="clip-empty">
				<ClipboardIcon size={24} strokeWidth={1.6} />
				<span>Copy something and it shows up here</span>
			</div>
		);
	}

	return (
		<div className="clip-wrap">
			<div className="clip-row" onWheel={onWheel}>
				<AnimatePresence initial={false} mode="popLayout">
					{items.map((item) => (
						<motion.div
							key={item.id}
							layout
							className={`clip-card ${item.kind} ${copiedId === item.id ? "copied" : ""}`}
							initial={{ opacity: 0, scale: 0.9 }}
							animate={{ opacity: 1, scale: 1 }}
							exit={{ opacity: 0, scale: 0.9, transition: { duration: 0.12 } }}
							transition={{ type: "spring", stiffness: 520, damping: 38 }}
							onClick={() => copy(item)}
							title="Click to copy"
						>
							{item.kind === "image" && item.thumbnail ? (
								<img src={item.thumbnail} alt="" draggable={false} className="clip-thumb" />
							) : item.kind === "files" ? (
								<div className="clip-files">
									<Files size={16} />
									<span>{item.preview}</span>
								</div>
							) : (
								<span className="clip-text">{item.preview}</span>
							)}
							<span className="clip-meta">
								{copiedId === item.id ? (
									<>
										<Check size={11} /> Copied
									</>
								) : (
									timeAgo(item.timestamp, now)
								)}
							</span>
							<div className="clip-actions" onClick={(e) => e.stopPropagation()}>
								<button
									type="button"
									title="Keep on the shelf"
									onClick={() =>
										invoke<ShelfPaste>("clipboard_to_shelf", { id: item.id })
											.then(onPinToShelf)
											.catch(console.error)
									}
								>
									<Pin size={11} />
								</button>
								<button
									type="button"
									title="Remove"
									onClick={() => invoke("clipboard_remove", { id: item.id }).catch(console.error)}
								>
									<X size={11} />
								</button>
							</div>
						</motion.div>
					))}
				</AnimatePresence>
			</div>
			<div className="clip-footer">
				<span>
					{items.length} {items.length === 1 ? "item" : "items"} · click to copy
				</span>
				<button type="button" onClick={() => invoke("clipboard_clear").catch(console.error)}>
					Clear
				</button>
			</div>
		</div>
	);
};
