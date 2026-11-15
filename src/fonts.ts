// Notch typefaces, bundled locally so they work offline and load instantly.
import "@fontsource-variable/inter";
import "@fontsource-variable/geist";
import "@fontsource-variable/outfit";
import "@fontsource-variable/space-grotesk";
import "@fontsource-variable/sora";
import "@fontsource-variable/plus-jakarta-sans";

export interface NotchFont {
	id: string;
	label: string;
	description: string;
	stack: string;
}

const FALLBACK = `"Segoe UI Variable", "Segoe UI", system-ui, sans-serif`;

export const NOTCH_FONTS: NotchFont[] = [
	{
		id: "inter",
		label: "Inter",
		description: "Neutral and crisp (default)",
		stack: `"Inter Variable", "Inter", ${FALLBACK}`
	},
	{
		id: "geist",
		label: "Geist",
		description: "Clean, technical, Vercel-style",
		stack: `"Geist Variable", ${FALLBACK}`
	},
	{
		id: "outfit",
		label: "Outfit",
		description: "Rounded geometric, friendly",
		stack: `"Outfit Variable", ${FALLBACK}`
	},
	{
		id: "space-grotesk",
		label: "Space Grotesk",
		description: "Quirky grotesk with character",
		stack: `"Space Grotesk Variable", ${FALLBACK}`
	},
	{
		id: "sora",
		label: "Sora",
		description: "Wide and futuristic",
		stack: `"Sora Variable", ${FALLBACK}`
	},
	{
		id: "plus-jakarta-sans",
		label: "Plus Jakarta Sans",
		description: "Elegant, soft curves",
		stack: `"Plus Jakarta Sans Variable", ${FALLBACK}`
	}
];

export const fontById = (id: string): NotchFont =>
	NOTCH_FONTS.find((f) => f.id === id) ?? NOTCH_FONTS[0];

/** Sets the notch typeface for this window. */
export function applyNotchFont(id: string) {
	document.documentElement.style.setProperty("--notch-font", fontById(id).stack);
}
