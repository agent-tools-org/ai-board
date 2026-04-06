import { esc, rel } from "./utils.js";

export function listCmp(a, b, sort, STR, PRN) {
	const k = sort.k;
	let va, vb;
	if (k === "priority") {
		va = PRN[a.priority] ?? 9;
		vb = PRN[b.priority] ?? 9;
	} else if (STR.includes(k)) {
		va = String(a[k] || "");
		vb = String(b[k] || "");
	} else {
		va = new Date(a[k] || 0).getTime();
		vb = new Date(b[k] || 0).getTime();
	}
	const c = va < vb ? -1 : va > vb ? 1 : 0;
	return c * sort.d;
}

export function renderList($, filtered, sort, STR, PRN, openDetail) {
	const rows = filtered().sort((a, b) => listCmp(a, b, sort, STR, PRN));
	const vl = $("#view-list");
	vl.innerHTML = `<table class="data"><thead><tr><th data-sort="id">ID</th><th data-sort="project">Project</th><th data-sort="priority">Priority</th><th data-sort="status">Status</th><th data-sort="title">Title</th><th data-sort="assignee">Assignee</th><th data-sort="updated_at">Updated</th></tr></thead><tbody>${rows
		.map(
			(it) =>
				`<tr data-id="${it.id}"><td>${esc(it.id)}</td><td>${it.project ? esc(it.project) : "—"}</td><td>${it.priority}</td><td>${it.status}</td><td>${esc(it.title)}</td><td>${it.assignee ? esc(it.assignee) : "—"}</td><td>${rel(it.updated_at)}</td></tr>`,
		)
		.join("")}</tbody></table>`;
	vl.querySelectorAll("th[data-sort]").forEach((th) => {
		th.onclick = () => {
			const k = th.dataset.sort;
			sort.d = sort.k === k ? -sort.d : -1;
			sort.k = k;
			renderList($, filtered, sort, STR, PRN, openDetail);
		};
	});
	for (const tr of vl.querySelectorAll("tbody tr"))
		tr.onclick = () => openDetail(tr.dataset.id);
}
