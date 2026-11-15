import React from "react";
import { motion } from "framer-motion";
import { Bluetooth, Headphones, Mic, Speaker } from "lucide-react";
import "./AudioDeviceCard.css";

export interface AudioToast {
	kind: "headphones" | "speaker" | "mic";
	/** Device name, e.g. "AirBuds Pro". */
	title: string;
	connected: boolean;
	bluetooth: boolean;
}

/** Height the collapsed notch grows by while the card shows (px). */
export const AUDIO_CARD_HEIGHT = 58;

const ICONS = { headphones: Headphones, speaker: Speaker, mic: Mic };

/**
 * "Device connected" card under the collapsed notch: device icon inside a ring
 * that sweeps closed on connect (open on disconnect), name and status, and the
 * device type on the right.
 */
export const AudioDeviceCard: React.FC<{ toast: AudioToast }> = ({ toast }) => {
	const Icon = ICONS[toast.kind];
	const circumference = 2 * Math.PI * 17;
	const status = `${toast.connected ? "Connected" : "Disconnected"}${toast.bluetooth ? " · Bluetooth" : ""}`;
	return (
		<motion.div
			className={`audio-card ${toast.connected ? "connected" : "disconnected"}`}
			style={{ height: AUDIO_CARD_HEIGHT }}
			initial={{ opacity: 0, y: -6, filter: "blur(4px)" }}
			animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
			exit={{ opacity: 0, y: -4, filter: "blur(4px)", transition: { duration: 0.15 } }}
			transition={{ type: "spring", stiffness: 420, damping: 32 }}
		>
			<div className="audio-card-ring">
				<svg className="audio-ring-svg" width="40" height="40" viewBox="0 0 40 40" aria-hidden>
					<circle cx="20" cy="20" r="17" className="audio-ring-track" />
					<motion.circle
						cx="20"
						cy="20"
						r="17"
						className="audio-ring-fill"
						strokeDasharray={circumference}
						initial={{ strokeDashoffset: toast.connected ? circumference : 0 }}
						animate={{ strokeDashoffset: toast.connected ? 0 : circumference }}
						transition={{ duration: 0.9, ease: [0.22, 1, 0.36, 1] }}
						transform="rotate(-90 20 20)"
					/>
				</svg>
				{toast.bluetooth ? <Bluetooth size={15} /> : <Icon size={15} />}
			</div>
			<div className="audio-card-text">
				<span className="audio-card-title">{toast.title}</span>
				<span className="audio-card-status">{status}</span>
			</div>
			<div className="audio-card-badge">
				<Icon size={16} />
			</div>
		</motion.div>
	);
};
