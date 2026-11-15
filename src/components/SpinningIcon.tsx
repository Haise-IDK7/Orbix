import React, { useEffect } from "react";
import { animate, motion, useMotionValue } from "framer-motion";

interface SpinningIconProps {
	src: string;
	spinning: boolean;
	className?: string;
	style?: React.CSSProperties;
	/** Seconds per clockwise turn while spinning. */
	period?: number;
}

/**
 * Image that turns clockwise while `spinning`, then eases to rest on the next
 * full turn instead of snapping back (so a Sharingan-style icon "settles").
 */
export const SpinningIcon: React.FC<SpinningIconProps> = ({
	src,
	spinning,
	className,
	style,
	period = 1.6
}) => {
	const rotate = useMotionValue(0);
	const loop = React.useRef<ReturnType<typeof animate> | null>(null);

	useEffect(() => {
		const from = rotate.get();
		if (spinning) {
			// Spin up gently, then keep turning at a constant speed.
			const controls = animate(rotate, from + 360, {
				duration: period * 1.25,
				ease: "easeIn",
				onComplete: () => {
					const start = rotate.get();
					loop.current = animate(rotate, [start, start + 360], {
						duration: period,
						ease: "linear",
						repeat: Infinity
					});
				}
			});
			return () => {
				controls.stop();
				loop.current?.stop();
			};
		}
		const rest = Math.ceil(from / 360) * 360;
		if (rest === from) return;
		const settle = animate(rotate, rest, { type: "spring", stiffness: 60, damping: 14 });
		return () => settle.stop();
	}, [spinning, period]);

	return (
		<motion.img
			src={src}
			alt="Start"
			className={className}
			style={{ ...style, rotate }}
			draggable={false}
		/>
	);
};
