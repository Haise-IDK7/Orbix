import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronLeft, RotateCw, Zap, X } from "lucide-react";
import { AppMemoryUsage, CloseAppResult, MemoryReport, OptimizeResult } from "./types";

interface PerformanceSubViewProps {
	onBack: () => void;
}

const formatMb = (mb: number) => (mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`);

const displayName = (exe: string) => exe.replace(/\.exe$/i, "");

export const PerformanceSubView: React.FC<PerformanceSubViewProps> = ({ onBack }) => {
	const [report, setReport] = useState<MemoryReport | null>(null);
	const [loading, setLoading] = useState(false);
	const [optimizing, setOptimizing] = useState(false);
	const [message, setMessage] = useState<string | null>(null);
	// Apps that ignored a polite close request and can now be force-quit.
	const [stubborn, setStubborn] = useState<Set<string>>(new Set());
	const [closing, setClosing] = useState<string | null>(null);

	const refresh = useCallback(async () => {
		setLoading(true);
		try {
			setReport(await invoke<MemoryReport>("get_memory_report"));
		} catch (err) {
			setMessage(String(err));
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		void refresh();
		const interval = setInterval(() => void refresh(), 5000);
		return () => clearInterval(interval);
	}, [refresh]);

	const handleOptimize = async () => {
		setOptimizing(true);
		setMessage(null);
		try {
			const result = await invoke<OptimizeResult>("optimize_memory");
			setMessage(
				result.freed_mb > 0
					? `Freed ${formatMb(result.freed_mb)} from ${result.processes_trimmed} processes`
					: "Memory was already in good shape"
			);
			await refresh();
		} catch (err) {
			setMessage(String(err));
		} finally {
			setOptimizing(false);
		}
	};

	const handleClose = async (app: AppMemoryUsage) => {
		const force = stubborn.has(app.name);
		setClosing(app.name);
		setMessage(null);
		try {
			const result = await invoke<CloseAppResult>("close_heavy_app", { name: app.name, force });
			setStubborn((prev) => {
				const next = new Set(prev);
				if (result.remaining > 0 && !force) next.add(app.name);
				else next.delete(app.name);
				return next;
			});
			if (result.remaining > 0) {
				setMessage(
					force
						? `${displayName(app.name)} could not be closed (it may need admin rights)`
						: `${displayName(app.name)} is still running. Click again to force quit.`
				);
			} else {
				setMessage(`Closed ${displayName(app.name)}`);
			}
			await refresh();
		} catch (err) {
			setMessage(String(err));
		} finally {
			setClosing(null);
		}
	};

	const usedPercent = report?.used_percent ?? 0;
	const usedMb = report ? report.total_mb - report.available_mb : 0;
	const level = usedPercent >= 85 ? "high" : usedPercent >= 65 ? "medium" : "low";

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
				<span className="cc-subview-title">Performance</span>
				<div className="cc-subview-header-actions">
					<button
						type="button"
						className={`cc-subview-icon-btn ${loading ? "spinning" : ""}`}
						onClick={(e) => {
							e.stopPropagation();
							void refresh();
						}}
						title="Refresh"
					>
						<RotateCw size={14} />
					</button>
				</div>
			</div>

			<div className="cc-subview-body">
				<div className="cc-perf-card">
					<div className="cc-perf-row">
						<span className="cc-perf-label">Memory in use</span>
						<span className="cc-perf-value">
							{report ? `${formatMb(usedMb)} / ${formatMb(report.total_mb)}` : "…"}
						</span>
					</div>
					<div className="cc-perf-bar">
						<div className={`cc-perf-bar-fill ${level}`} style={{ width: `${usedPercent}%` }} />
					</div>
					<button
						type="button"
						className="cc-subview-primary-btn cc-perf-optimize"
						disabled={optimizing}
						onClick={(e) => {
							e.stopPropagation();
							void handleOptimize();
						}}
						title="Trims idle memory from running apps so Windows has more free RAM"
					>
						<Zap size={13} />
						{optimizing ? "Freeing memory…" : "Free up RAM"}
					</button>
					{message && <span className="cc-perf-message">{message}</span>}
				</div>

				<span className="cc-perf-section-title">Using the most memory</span>
				<div className="cc-perf-apps">
					{report?.top_apps.map((app) => (
						<div key={app.name} className="cc-perf-app-row">
							<div className="cc-perf-app-info">
								<span className="cc-perf-app-name">{displayName(app.name)}</span>
								<span className="cc-perf-app-meta">
									{formatMb(app.memory_mb)}
									{app.process_count > 1 ? ` · ${app.process_count} processes` : ""}
								</span>
							</div>
							<button
								type="button"
								className={`cc-perf-close-btn ${stubborn.has(app.name) ? "force" : ""}`}
								disabled={closing === app.name}
								onClick={(e) => {
									e.stopPropagation();
									void handleClose(app);
								}}
								title={stubborn.has(app.name) ? "Force quit (unsaved work is lost)" : "Close app"}
							>
								{closing === app.name ? (
									"…"
								) : stubborn.has(app.name) ? (
									"Force quit"
								) : (
									<X size={12} />
								)}
							</button>
						</div>
					))}
				</div>
			</div>
		</div>
	);
};
