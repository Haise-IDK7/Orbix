// Lock screen clock typefaces: the notch fonts plus display faces made for
// very large numerals. All bundled, so they work offline.
import "@fontsource-variable/oswald";
import "@fontsource/bebas-neue";
import "@fontsource-variable/playfair-display";
import "@fontsource-variable/unbounded";
import { NOTCH_FONTS, type NotchFont } from "../../fonts";

export interface LockFont extends NotchFont {
	/** Whether the weight slider applies (variable fonts). */
	variable: boolean;
}

export const LOCK_FONTS: LockFont[] = [
	{
		id: "oswald",
		label: "Oswald",
		description: "Tall and condensed, like iOS",
		stack: `"Oswald Variable", sans-serif`,
		variable: true
	},
	{
		id: "bebas-neue",
		label: "Bebas Neue",
		description: "Bold poster capitals",
		stack: `"Bebas Neue", sans-serif`,
		variable: false
	},
	{
		id: "playfair-display",
		label: "Playfair",
		description: "Elegant high-contrast serif",
		stack: `"Playfair Display Variable", serif`,
		variable: true
	},
	{
		id: "unbounded",
		label: "Unbounded",
		description: "Wide and futuristic",
		stack: `"Unbounded Variable", sans-serif`,
		variable: true
	},
	...NOTCH_FONTS.map((f) => ({ ...f, variable: true }))
];

export const lockFontById = (id: string): LockFont =>
	LOCK_FONTS.find((f) => f.id === id) ?? LOCK_FONTS[0];
