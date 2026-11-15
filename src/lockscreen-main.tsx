import React from "react";
import ReactDOM from "react-dom/client";
import { LockScreen } from "./components/lockscreen/LockScreen";

document.addEventListener("contextmenu", (e) => e.preventDefault());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
	<React.StrictMode>
		<LockScreen />
	</React.StrictMode>
);
