import React from "react";
import { motion } from "framer-motion";

interface PanelDotsProps {
	count: number;
	index: number;
	onSelect: (index: number) => void;
}

/** Page indicator for the notch panels; the active page stretches into a pill. */
export const PanelDots: React.FC<PanelDotsProps> = ({ count, index, onSelect }) => (
	<div className="panel-dots" onClick={(e) => e.stopPropagation()}>
		{Array.from({ length: count }, (_, i) => (
			<motion.button
				key={i}
				type="button"
				className={`panel-dot ${i === index ? "active" : ""}`}
				animate={{ width: i === index ? 14 : 4 }}
				transition={{ type: "spring", stiffness: 500, damping: 34 }}
				onClick={() => onSelect(i)}
				aria-label={`Panel ${i + 1} of ${count}`}
			/>
		))}
	</div>
);
