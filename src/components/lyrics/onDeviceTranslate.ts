// Translation with the translator built into WebView2/Edge (Edge 148+):
// models run on this PC, so there's no quota and nothing leaves the device.
// Feature-detected; returns null whenever it can't help, and the caller
// falls back to the cloud engines.

interface BuiltInTranslator {
	translate(text: string): Promise<string>;
}

type Availability = "unavailable" | "downloadable" | "downloading" | "available";

interface BuiltIns {
	Translator?: {
		availability(o: { sourceLanguage: string; targetLanguage: string }): Promise<Availability>;
		create(o: { sourceLanguage: string; targetLanguage: string }): Promise<BuiltInTranslator>;
	};
	LanguageDetector?: {
		availability(): Promise<Availability>;
		create(): Promise<{
			detect(text: string): Promise<{ detectedLanguage: string; confidence: number }[]>;
			destroy?: () => void;
		}>;
	};
}

const builtIns = globalThis as unknown as BuiltIns;

/** Whether this WebView exposes the built-in Translator at all. */
export const onDeviceTranslatorPresent = () => typeof builtIns.Translator?.create === "function";

/** One translator per language pair for the whole session. */
const translators = new Map<string, Promise<BuiltInTranslator>>();

async function detectLanguage(text: string): Promise<string | null> {
	const LD = builtIns.LanguageDetector;
	if (!LD) return null;
	try {
		if ((await LD.availability()) === "unavailable") return null;
		const detector = await LD.create();
		const [top] = await detector.detect(text);
		detector.destroy?.();
		return top && top.confidence >= 0.5 && top.detectedLanguage !== "und"
			? top.detectedLanguage.slice(0, 2).toLowerCase()
			: null;
	} catch {
		return null;
	}
}

export interface DeviceTranslation {
	lines: string[];
	source: string;
}

/**
 * Translates line by line with the built-in model. The first use of a
 * language pair downloads its model; Windows requires that to start from a
 * click, which the translate button provides.
 */
export async function translateOnDevice(
	lines: string[],
	target: string
): Promise<DeviceTranslation | null> {
	const T = builtIns.Translator;
	if (!T) return null;
	const unique = [...new Set(lines.map((l) => l.trim()).filter(Boolean))];
	if (unique.length === 0) return null;
	const source = await detectLanguage(unique.join("\n").slice(0, 2000));
	if (!source) return null;
	if (source === target) return { lines, source };
	try {
		if (
			(await T.availability({ sourceLanguage: source, targetLanguage: target })) === "unavailable"
		)
			return null;
		const pair = `${source}>${target}`;
		let pending = translators.get(pair);
		if (!pending) {
			pending = T.create({ sourceLanguage: source, targetLanguage: target });
			translators.set(pair, pending);
			pending.catch(() => translators.delete(pair));
		}
		const translator = await pending;
		const out = new Map<string, string>();
		for (const line of unique) out.set(line, (await translator.translate(line)).trim());
		return { lines: lines.map((l) => out.get(l.trim()) ?? ""), source };
	} catch {
		return null;
	}
}
