import React, { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Check, ListChecks, Plus, Trash2, X } from "lucide-react";
import "./Focus.css";

interface Task {
	id: string;
	text: string;
	done: boolean;
	/** Local date (YYYY-MM-DD) the task was completed. */
	doneOn?: string;
}

const STORAGE_KEY = "bloom-tasks";
const MAX_TASK_LENGTH = 120;
const today = () => new Date().toLocaleDateString("en-CA");

/** Tasks completed on an earlier day are cleared when the list loads. */
function load(): Task[] {
	try {
		const parsed = JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]");
		if (!Array.isArray(parsed)) return [];
		return parsed.filter(
			(t: Task) =>
				typeof t?.id === "string" && typeof t.text === "string" && (!t.done || t.doneOn === today())
		);
	} catch {
		return [];
	}
}

let tasks = load();
const listeners = new Set<() => void>();
const setTasks = (next: Task[]) => {
	tasks = next;
	localStorage.setItem(STORAGE_KEY, JSON.stringify(tasks));
	listeners.forEach((l) => l());
};
const subscribe = (l: () => void) => {
	listeners.add(l);
	return () => listeners.delete(l);
};

/** Small daily to-do list: add, tick off, and remove tasks. */
export const TasksView: React.FC = () => {
	const list = useSyncExternalStore(subscribe, () => tasks);
	const [adding, setAdding] = useState(false);
	const [draft, setDraft] = useState("");
	const inputRef = useRef<HTMLInputElement>(null);

	useEffect(() => {
		if (adding) inputRef.current?.focus();
	}, [adding]);

	const doneCount = list.filter((t) => t.done).length;

	const add = () => {
		const text = draft.trim().slice(0, MAX_TASK_LENGTH);
		if (text)
			setTasks([
				...list,
				{ id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, text, done: false }
			]);
		setDraft("");
		inputRef.current?.focus();
	};

	const toggle = (id: string) =>
		setTasks(
			list.map((t) =>
				t.id === id ? { ...t, done: !t.done, doneOn: t.done ? undefined : today() } : t
			)
		);

	// Open tasks first, then completed, each in creation order.
	const ordered = [...list.filter((t) => !t.done), ...list.filter((t) => t.done)];

	return (
		<motion.div
			className="focus-panel"
			onClick={(e) => e.stopPropagation()}
			initial={{ opacity: 0, y: 6 }}
			animate={{ opacity: 1, y: 0 }}
			exit={{ opacity: 0, transition: { duration: 0.1 } }}
			transition={{ type: "spring", stiffness: 420, damping: 34 }}
		>
			<div className="panel-title-row">
				<span className="panel-title">Tasks</span>
			</div>
			<div className="tasks-body">
				<div className="tasks-left">
					<span className="tasks-count">
						{doneCount}/{list.length}
					</span>
					<span className="tasks-caption">Done today</span>
					<div className="tasks-meter">
						<div style={{ transform: `scaleX(${list.length ? doneCount / list.length : 0})` }} />
					</div>
					<div className="tasks-buttons">
						<button type="button" className="tasks-btn primary" onClick={() => setAdding(true)}>
							<Plus size={13} /> Add task
						</button>
						{doneCount > 0 && (
							<button
								type="button"
								className="panel-icon-btn"
								onClick={() => setTasks(list.filter((t) => !t.done))}
								title="Clear completed tasks"
							>
								<Trash2 size={12} />
							</button>
						)}
					</div>
				</div>
				<div className="tasks-card">
					{adding && (
						<div className="tasks-input-row">
							<input
								ref={inputRef}
								value={draft}
								maxLength={MAX_TASK_LENGTH}
								placeholder="New task…"
								onChange={(e) => setDraft(e.target.value)}
								onKeyDown={(e) => {
									if (e.key === "Enter") add();
									if (e.key === "Escape") {
										setAdding(false);
										setDraft("");
									}
								}}
							/>
							<button type="button" className="panel-icon-btn" onClick={add} title="Add">
								<Check size={13} />
							</button>
						</div>
					)}
					{ordered.length === 0 && !adding ? (
						<div className="tasks-empty">
							<ListChecks size={26} strokeWidth={1.6} />
							<span>No tasks yet.</span>
						</div>
					) : (
						<div className="tasks-list">
							<AnimatePresence initial={false}>
								{ordered.map((task) => (
									<motion.div
										key={task.id}
										layout="position"
										className={`task-row ${task.done ? "done" : ""}`}
										initial={{ opacity: 0, y: -4 }}
										animate={{ opacity: 1, y: 0 }}
										exit={{ opacity: 0, transition: { duration: 0.12 } }}
										transition={{ type: "spring", stiffness: 500, damping: 38 }}
									>
										<button
											type="button"
											className="task-check"
											onClick={() => toggle(task.id)}
											aria-label={task.done ? "Mark as not done" : "Mark as done"}
										>
											{task.done && <Check size={11} strokeWidth={3} />}
										</button>
										<span className="task-text" onClick={() => toggle(task.id)}>
											{task.text}
										</span>
										<button
											type="button"
											className="task-remove"
											onClick={() => setTasks(list.filter((t) => t.id !== task.id))}
											aria-label="Delete task"
										>
											<X size={12} />
										</button>
									</motion.div>
								))}
							</AnimatePresence>
						</div>
					)}
				</div>
			</div>
		</motion.div>
	);
};
