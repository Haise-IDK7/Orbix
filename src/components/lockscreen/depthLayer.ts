// Subject cut-out for the lock screen's depth effect (subject in front of the
// clock). Uses U²-Net-P (Apache-2.0, ~4.5 MB), a small salient-object model,
// run once per wallpaper with ONNX Runtime Web in a worker thread.
import * as ort from "onnxruntime-web/wasm";
import wasmUrl from "onnxruntime-web/ort-wasm-simd-threaded.wasm?url";

ort.env.wasm.wasmPaths = { wasm: wasmUrl };
ort.env.wasm.numThreads = 1;
// Inference happens in a worker so the UI never stalls.
ort.env.wasm.proxy = true;

const SIZE = 320;
const MEAN = [0.485, 0.456, 0.406];
const STD = [0.229, 0.224, 0.225];

let session: Promise<ort.InferenceSession> | null = null;
const getSession = () =>
	(session ??= ort.InferenceSession.create("/models/u2netp.onnx", {
		executionProviders: ["wasm"]
	}));

type Source = HTMLImageElement | HTMLCanvasElement;
const sizeOf = (img: Source) =>
	img instanceof HTMLImageElement ? [img.naturalWidth, img.naturalHeight] : [img.width, img.height];

/** Foreground probability per pixel (0..1) on a 320×320 grid. */
async function subjectMask(img: Source): Promise<Float32Array> {
	const canvas = document.createElement("canvas");
	canvas.width = canvas.height = SIZE;
	const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
	ctx.drawImage(img, 0, 0, SIZE, SIZE);
	const { data } = ctx.getImageData(0, 0, SIZE, SIZE);
	const input = new Float32Array(3 * SIZE * SIZE);
	for (let i = 0; i < SIZE * SIZE; i++) {
		for (let c = 0; c < 3; c++) {
			input[c * SIZE * SIZE + i] = (data[i * 4 + c] / 255 - MEAN[c]) / STD[c];
		}
	}
	const s = await getSession();
	const result = await s.run({
		[s.inputNames[0]]: new ort.Tensor("float32", input, [1, 3, SIZE, SIZE])
	});
	const out = result[s.outputNames[0]].data as Float32Array;
	let min = Infinity;
	let max = -Infinity;
	for (const v of out) {
		if (v < min) min = v;
		if (v > max) max = v;
	}
	const range = max - min || 1;
	return out.map((v) => (v - min) / range);
}

/** Rectangle that cover-fits an image of iw×ih into w×h, centered. */
export function coverRect(iw: number, ih: number, w: number, h: number) {
	const scale = Math.max(w / iw, h / ih);
	const dw = iw * scale;
	const dh = ih * scale;
	return { x: (w - dw) / 2, y: (h - dh) / 2, w: dw, h: dh };
}

/**
 * The wallpaper, cover-fitted to w×h, with everything but its main subject
 * made transparent. Draw it above the clock and above the plain wallpaper.
 */
export async function subjectLayer(img: Source, w: number, h: number): Promise<HTMLCanvasElement> {
	const mask = await subjectMask(img);
	const maskCanvas = document.createElement("canvas");
	maskCanvas.width = maskCanvas.height = SIZE;
	const mctx = maskCanvas.getContext("2d")!;
	const pixels = mctx.createImageData(SIZE, SIZE);
	for (let i = 0; i < mask.length; i++) {
		// Soft threshold keeps hair/petal edges smooth without halo.
		const a = Math.min(1, Math.max(0, (mask[i] - 0.3) / 0.4));
		pixels.data[i * 4] = pixels.data[i * 4 + 1] = pixels.data[i * 4 + 2] = 255;
		pixels.data[i * 4 + 3] = Math.round(a * 255);
	}
	mctx.putImageData(pixels, 0, 0);

	const [iw, ih] = sizeOf(img);
	const rect = coverRect(iw, ih, w, h);
	const out = document.createElement("canvas");
	out.width = w;
	out.height = h;
	const ctx = out.getContext("2d")!;
	ctx.imageSmoothingQuality = "high";
	ctx.drawImage(img, rect.x, rect.y, rect.w, rect.h);
	ctx.globalCompositeOperation = "destination-in";
	ctx.drawImage(maskCanvas, rect.x, rect.y, rect.w, rect.h);
	return out;
}
