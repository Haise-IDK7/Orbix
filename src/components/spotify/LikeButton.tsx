import React from "react";
import { motion } from "framer-motion";
import { Heart } from "lucide-react";
import type { LikeStatus } from "./useSpotifyLike";

interface LikeButtonProps {
	liked: boolean;
	onToggle: () => void;
	status?: LikeStatus;
	hint?: string;
	size?: number;
}

/** Spotify "Liked Songs" toggle with a small pop when liking. */
export const LikeButton: React.FC<LikeButtonProps> = ({
	liked,
	onToggle,
	status = "ready",
	hint,
	size = 15
}) => (
	<motion.button
		type="button"
		className={`like-btn ${liked ? "liked" : ""} ${status !== "ready" ? `like-${status}` : ""}`}
		onClick={(e) => {
			e.stopPropagation();
			onToggle();
		}}
		whileTap={{ scale: 0.85 }}
		title={hint ?? (liked ? "Remove from Liked Songs" : "Save to Liked Songs")}
		aria-label={hint}
	>
		<motion.span
			key={liked ? "on" : "off"}
			initial={liked ? { scale: 0.6 } : false}
			animate={{ scale: [0.6, 1.25, 1] }}
			transition={{ duration: 0.35, ease: "easeOut" }}
			style={{ display: "flex" }}
		>
			<Heart size={size} fill={liked ? "currentColor" : "none"} strokeWidth={2} />
		</motion.span>
	</motion.button>
);
