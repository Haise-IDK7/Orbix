import React, { useLayoutEffect, useRef, useState } from "react";
import "./Glass.css";

interface GlassLayerProps {
	/**
	 * Screen edge the host window is attached to: the notch window starts at the
	 * top of the monitor, the dock window ends at its bottom.
	 */
	anchor: "top" | "bottom";
	/** CSS zoom or transform scale applied to the host, so screen pixels can be mapped to local ones. */
	scale?: number;
}

const REMEASURE_MS = 1500;

/**
 * Frosted-glass background: the blurred wallpaper from `useGlassBackdrop`,
 * positioned so it lines up with the real desktop behind the window, plus a
 * tint, a top sheen and a hairline border. Place it as the first child of a
 * rounded element with `isolation: isolate`.
 */
export const GlassLayer: React.FC<GlassLayerProps> = ({ anchor, scale = 1 }) => {
	const ref = useRef<HTMLDivElement>(null);
	// Distance (screen px) from the layer to the anchored screen edge.
	const [edgeOffset, setEdgeOffset] = useState(0);

	useLayoutEffect(() => {
		const el = ref.current;
		if (!el) return;
		const measure = () => {
			const rect = el.getBoundingClientRect();
			const next = Math.round(anchor === "top" ? rect.top : window.innerHeight - rect.bottom);
			setEdgeOffset((prev) => (Math.abs(prev - next) > 1 ? next : prev));
		};
		measure();
		// Entry/peek animations move the host with transforms, which neither
		// ResizeObserver nor resize events report, so also re-check periodically.
		const observer = new ResizeObserver(measure);
		observer.observe(el);
		const interval = setInterval(measure, REMEASURE_MS);
		window.addEventListener("resize", measure);
		return () => {
			observer.disconnect();
			clearInterval(interval);
			window.removeEventListener("resize", measure);
		};
	}, [anchor]);

	// Both windows span the monitor and center their content, so "50%" lines
	// the image center up with the screen center at any width.
	const shift = `(var(--glass-crop-y, 0px) + ${edgeOffset}px) / ${scale}`;
	const positionY = anchor === "top" ? `calc(-1 * ${shift})` : `calc(100% + ${shift})`;

	return (
		<div
			ref={ref}
			className="glass-layer"
			style={{
				backgroundPosition: `50% ${positionY}`,
				backgroundSize: `calc(var(--glass-bg-w, 100%) / ${scale}) calc(var(--glass-bg-h, 100%) / ${scale})`
			}}
			aria-hidden
		/>
	);
};
