// Wallpaper-aware colors for the lock screen clock.

type RGB = [number, number, number];

function rgbToHsl([r, g, b]: RGB): [number, number, number] {
	r /= 255;
	g /= 255;
	b /= 255;
	const max = Math.max(r, g, b);
	const min = Math.min(r, g, b);
	const l = (max + min) / 2;
	if (max === min) return [0, 0, l];
	const d = max - min;
	const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
	const h =
		max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
	return [h * 60, s, l];
}

export const hsl = (h: number, s: number, l: number, a = 1) =>
	`hsla(${Math.round(h)}, ${Math.round(s * 100)}%, ${Math.round(l * 100)}%, ${a})`;

export interface ClockPalette {
	/** Color for the clock, tuned for contrast with what's behind it. */
	clock: string;
	/** Softer companion for dates and widgets. */
	accent: string;
	/** True when the area behind the clock is dark. */
	darkBehindClock: boolean;
}

/**
 * Samples the area where the clock sits (top-middle of the screen for "top",
 * the middle for "center") and derives a color in the same hue family: a light
 * tint over dark scenes, a deep shade over bright ones — the way iOS tints
 * its lock screen clock.
 */
export function clockPalette(img: HTMLImageElement, position: "top" | "center"): ClockPalette {
	const W = 64;
	const H = 36;
	const canvas = document.createElement("canvas");
	canvas.width = W;
	canvas.height = H;
	const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
	// Cover-fit like the background, so samples match what's on screen.
	const scale = Math.max(W / img.naturalWidth, H / img.naturalHeight);
	const dw = img.naturalWidth * scale;
	const dh = img.naturalHeight * scale;
	ctx.drawImage(img, (W - dw) / 2, (H - dh) / 2, dw, dh);
	const y0 = position === "top" ? Math.round(H * 0.08) : Math.round(H * 0.3);
	const { data } = ctx.getImageData(
		Math.round(W * 0.2),
		y0,
		Math.round(W * 0.6),
		Math.round(H * 0.4)
	);

	let lum = 0;
	let count = 0;
	// Hue from the most saturated quarter of pixels, so a dull sky doesn't win.
	const samples: { hsl: [number, number, number]; score: number }[] = [];
	for (let i = 0; i < data.length; i += 4) {
		const rgb: RGB = [data[i], data[i + 1], data[i + 2]];
		lum += 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
		count++;
		const c = rgbToHsl(rgb);
		samples.push({ hsl: c, score: c[1] * (1 - Math.abs(c[2] - 0.5)) });
	}
	samples.sort((a, b) => b.score - a.score);
	const top = samples.slice(0, Math.max(1, Math.floor(samples.length / 4)));
	// Circular mean of hue.
	let x = 0;
	let y = 0;
	let sat = 0;
	for (const {
		hsl: [h, s]
	} of top) {
		x += Math.cos((h * Math.PI) / 180) * s;
		y += Math.sin((h * Math.PI) / 180) * s;
		sat += s;
	}
	const hue = ((Math.atan2(y, x) * 180) / Math.PI + 360) % 360;
	const saturation = Math.min(0.55, sat / top.length);
	const dark = lum / count / 255 < 0.5;

	return dark
		? {
				clock: hsl(hue, saturation * 0.6, 0.86, 0.95),
				accent: hsl(hue, saturation * 0.5, 0.92, 0.9),
				darkBehindClock: true
			}
		: {
				clock: hsl(hue, Math.max(0.35, saturation), 0.2, 0.92),
				accent: hsl(hue, saturation, 0.18, 0.85),
				darkBehindClock: false
			};
}

/** A blurred, downscaled copy for the frosted "glass" digits. */
export function blurredCopy(img: HTMLImageElement, radius = 18): string {
	const canvas = document.createElement("canvas");
	const scale = 640 / Math.max(img.naturalWidth, img.naturalHeight);
	canvas.width = Math.round(img.naturalWidth * scale);
	canvas.height = Math.round(img.naturalHeight * scale);
	const ctx = canvas.getContext("2d")!;
	ctx.filter = `blur(${radius}px) saturate(1.4)`;
	// Overdraw the edges so the blur doesn't fade them to transparent.
	ctx.drawImage(img, -radius, -radius, canvas.width + radius * 2, canvas.height + radius * 2);
	return canvas.toDataURL("image/jpeg", 0.85);
}
