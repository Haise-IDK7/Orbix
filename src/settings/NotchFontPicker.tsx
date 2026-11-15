import { Type } from "lucide-react";
import { NOTCH_FONTS } from "../fonts";
import { useStoredSetting } from "../hooks/useStoredSetting";

/** Typeface choice for the notch, previewed with the clock as it will look. */
export function NotchFontPicker() {
	const [fontId, setFontId] = useStoredSetting("bloom-notch-font", "inter");

	return (
		<div className="font-picker">
			<div className="font-picker-title">
				<Type size={13} />
				<span>Notch Font</span>
			</div>
			<div className="font-picker-grid">
				{NOTCH_FONTS.map((font) => (
					<button
						type="button"
						key={font.id}
						className={`font-option ${fontId === font.id ? "active" : ""}`}
						style={{ fontFamily: font.stack }}
						onClick={() => setFontId(font.id)}
					>
						<span className="font-option-sample">9:41</span>
						<span className="font-option-name">{font.label}</span>
						<span className="font-option-desc">{font.description}</span>
					</button>
				))}
			</div>
		</div>
	);
}
