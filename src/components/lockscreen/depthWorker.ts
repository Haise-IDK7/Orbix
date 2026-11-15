// Runs U²-Net-P on one 320×320 image and posts the raw mask back. The page
// terminates this worker afterwards, which frees the WebAssembly heap and the
// model (tens of MB) that would otherwise stay in the lock screen's memory.
import * as ort from "onnxruntime-web/wasm";
import wasmUrl from "onnxruntime-web/ort-wasm-simd-threaded.wasm?url";

ort.env.wasm.wasmPaths = { wasm: wasmUrl };
ort.env.wasm.numThreads = 1;

const SIZE = 320;

self.onmessage = async (e: MessageEvent<{ input: Float32Array; modelUrl: string }>) => {
	try {
		const session = await ort.InferenceSession.create(e.data.modelUrl, {
			executionProviders: ["wasm"]
		});
		const result = await session.run({
			[session.inputNames[0]]: new ort.Tensor("float32", e.data.input, [1, 3, SIZE, SIZE])
		});
		const out = new Float32Array(result[session.outputNames[0]].data as Float32Array);
		await session.release();
		(self as unknown as Worker).postMessage({ out }, [out.buffer]);
	} catch (err) {
		(self as unknown as Worker).postMessage({ error: String(err) });
	}
};
