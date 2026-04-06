import { artifactsSectionHtml, bindArtifactsPanel, gateWarningHtml } from "./artifacts.js";
import { renderList } from "./list.js";
import { connectSse } from "./sse.js";
import { defaultProject, esc, multiProj, rel } from "./utils.js";

(() => {
	const COLS = ["backlog", "ready", "active", "review", "done", "blocked", "rejected"];
	const PRIOS = ["critical", "high", "medium", "low"];
	const PRN = { critical: 0, high: 1, medium: 2, low: 3 };
	const STR = ["id", "project", "title", "status", "assignee"];
	const items = new Map();
	const filters = {
		st: new Set(),
		pr: new Set(),
		lb: new Set(),
		pj: new Set(),
	};
	let view = "board";
	const sort = { k: "updated_at", d: -1 };
	let dragId = null;
	const $ = (s, r = document) => r.querySelector(s);
	async function api(m, p, b) {
		const o = { method: m, headers: { "Content-Type": "application/json" } };
		if (b !== undefined) o.body = JSON.stringify(b);
		const res = await fetch(p, o);
		const t = await res.text();
		if (!res.ok) throw new Error(t || res.statusText);
		return t ? JSON.parse(t) : null;
	}
	function filtered() {
		return [...items.values()].filter((it) => {
			if (filters.st.size && !filters.st.has(it.status)) return false;
			if (filters.pr.size && !filters.pr.has(it.priority)) return false;
			if (filters.lb.size && !(it.labels || []).some((l) => filters.lb.has(l)))
				return false;
			if (filters.pj.size && !filters.pj.has(it.project ?? "")) return false;
			return true;
		});
	}
	function upsertItem(it) {
		items.set(it.id, it);
	}
	async function loadItems() {
		let u = "/api/items?limit=2000";
		if (filters.pj.size === 1)
			u += `&project=${encodeURIComponent([...filters.pj][0])}`;
		const list = await api("GET", u);
		items.clear();
		for (const it of list) items.set(it.id, it);
		render();
	}
	async function refreshItem(id) {
		try {
			const data = await api("GET", `/api/items/${encodeURIComponent(id)}`);
			upsertItem(data.item);
			render();
			if ($("#detail-panel")?.dataset.id === id) openDetail(id);
		} catch (_) {
			items.delete(id);
			render();
			closeDetail();
		}
	}
	function renderFilters() {
		const labels = [...new Set([...items.values()].flatMap((i) => i.labels))].sort();
		const projects = [
			...new Set([...items.values()].map((i) => i.project).filter(Boolean)),
		].sort();
		const pill = (type, val, lab) =>
			`<button type="button" class="pill ${filters[type].has(val) ? "is-on" : ""}" data-flt="${type}" data-v="${val}">${lab}</button>`;
		const fb = $("#filter-bar");
		fb.innerHTML = `<div class="group"><label>Status</label>${COLS.map((s) => pill("st", s, s)).join("")}</div><div class="group"><label>Priority</label>${PRIOS.map((p) => pill("pr", p, p)).join("")}</div><div class="group"><label>Label</label>${labels.map((l) => pill("lb", l, l)).join("")}</div><div class="group"><label>Project</label>${projects.map((p) => pill("pj", p, p)).join("")}</div>`;
		fb.onclick = async (e) => {
			const b = e.target.closest("[data-flt]");
			if (!b) return;
			const t = b.dataset.flt;
			const v = b.dataset.v;
			filters[t].has(v) ? filters[t].delete(v) : filters[t].add(v);
			if (t === "pj") await loadItems();
			else render();
		};
	}
	function cardHtml(it, showPb) {
		const pri = it.priority || "medium";
		const pb =
			showPb && it.project
				? `<span class="project-badge">${esc(it.project)}</span>`
				: "";
		return `<article class="card pri-${pri}" draggable="true" data-id="${it.id}"><p class="card-title">${esc(it.title)}</p><div class="card-meta"><span class="badge badge-pri-${pri}">${pri}</span>${pb}${(it.labels || []).map((l) => `<span class="label-pill">${esc(l)}</span>`).join("")}<span>${it.assignee ? esc(it.assignee) : "—"}</span><span>${rel(it.updated_at)}</span></div></article>`;
	}
	function renderBoard() {
		const list = filtered();
		const by = Object.fromEntries(COLS.map((c) => [c, []]));
		const mp = multiProj(items);
		for (const it of list) {
			if (!by[it.status]) by[it.status] = [];
			by[it.status].push(it);
		}
		for (const c of COLS) by[c].sort((a, b) => a.position - b.position);
		$("#view-board").innerHTML = `<div class="board">${COLS.map(
			(st) =>
				`<div class="column" data-status="${st}"><h2>${st}</h2><div class="column-body">${(by[st] || []).map((i) => cardHtml(i, mp)).join("")}</div></div>`,
		).join("")}</div>`;
		bindBoardDnD();
	}
	function bindBoardDnD() {
		$("#view-board")
			.querySelectorAll(".card")
			.forEach((card) => {
				card.ondragstart = (e) => {
					dragId = card.dataset.id;
					card.classList.add("dragging");
					e.dataTransfer.setData("text/plain", dragId);
					e.dataTransfer.effectAllowed = "move";
				};
				card.ondragend = () => card.classList.remove("dragging");
				card.onclick = (e) => {
					e.stopPropagation();
					openDetail(card.dataset.id);
				};
			});
		$("#view-board")
			.querySelectorAll(".column")
			.forEach((col) => {
				col.ondragover = (e) => {
					e.preventDefault();
					col.classList.add("drag-over");
				};
				col.ondragleave = () => col.classList.remove("drag-over");
				col.ondrop = async (e) => {
					e.preventDefault();
					col.classList.remove("drag-over");
					const id = e.dataTransfer.getData("text/plain") || dragId;
					if (!id) return;
					const body = col.querySelector(".column-body");
					const idx = [...body.querySelectorAll(".card")].filter(
						(c) => c.dataset.id !== id,
					).length;
					try {
						await api("PATCH", "/api/items/reorder", [
							{ id, position: (idx + 1) * 1000, status: col.dataset.status },
						]);
						await refreshItem(id);
					} catch (err) {
						alert(err.message || String(err));
					}
				};
			});
	}
	function closeDetail() {
		$("#panel-root").innerHTML = "";
	}
	async function openDetail(id) {
		const data = await api("GET", `/api/items/${encodeURIComponent(id)}`);
		const it = data.item;
		let arts = [];
		let artErr = "";
		try {
			arts = await api("GET", `/api/items/${encodeURIComponent(id)}/artifacts`);
		} catch (e) {
			artErr = e.message || String(e);
		}
		const gw = artErr ? "" : gateWarningHtml(it.status, arts);
		const artSec = artifactsSectionHtml(arts, artErr);
		const evs = data.events || [];
		const review = it.status === "review";
		const pr = PRIOS.map(
			(p) => `<option ${p === it.priority ? "selected" : ""}>${p}</option>`,
		).join("");
		const st = COLS.map(
			(s) => `<option ${s === it.status ? "selected" : ""}>${s}</option>`,
		).join("");
		const hist = evs.slice().reverse().map(
			(ev) =>
				`<div class="event"><strong>${esc(ev.action)}</strong> · ${esc(ev.actor)} · ${rel(ev.created_at)}${ev.detail ? ` — ${esc(ev.detail)}` : ""}</div>`,
		).join("");
		const rev = review
			? `<button type="button" class="btn-primary" id="f-ap">Approve</button><button type="button" class="btn-ghost" id="f-rj">Reject</button>`
			: "";
		$("#panel-root").innerHTML =
			`<div class="overlay-backdrop" id="db"></div><aside class="side-panel" id="detail-panel" data-id="${it.id}"><div class="panel-head"><div><strong>${esc(it.id)}</strong></div><button type="button" id="px" aria-label="Close">×</button></div><div class="panel-body"><div class="panel-row"><label>Title</label><input type="text" id="f-title" value="${esc(it.title)}" /></div><div class="panel-row"><label>Description</label><textarea id="f-desc">${esc(it.description || "")}</textarea></div><div class="panel-row"><label>Project</label><input type="text" id="f-proj" value="${esc(it.project || "")}" /></div><div class="panel-row"><label>Priority</label><select id="f-pri">${pr}</select></div><div class="panel-row"><label>Status</label><select id="f-st">${st}</select></div><div class="panel-actions"><button type="button" class="btn-primary" id="f-save">Save</button>${rev}<button type="button" class="btn-danger" id="f-del">Delete</button></div><div class="events"><h3>History</h3>${hist}</div>${gw}${artSec}</div></aside>`;
		const close = () => closeDetail();
		$("#db").onclick = close;
		$("#px").onclick = close;
		$("#f-save").onclick = async () => {
			try {
				await api("PATCH", `/api/items/${encodeURIComponent(id)}`, {
					title: $("#f-title").value,
					description: $("#f-desc").value,
					project: $("#f-proj").value.trim() || undefined,
					priority: $("#f-pri").value,
					status: $("#f-st").value,
				});
				await refreshItem(id);
				close();
			} catch (e) {
				alert(e.message || String(e));
			}
		};
		bindArtifactsPanel($, id, api, () => openDetail(id));
		$("#f-del").onclick = async () => {
			if (!confirm("Delete this item?")) return;
			await api("DELETE", `/api/items/${encodeURIComponent(id)}`);
			items.delete(id);
			render();
			close();
		};
		if (review) {
			$("#f-ap").onclick = async () => {
				await api("POST", `/api/items/${encodeURIComponent(id)}/approve`);
				await refreshItem(id);
				close();
			};
			$("#f-rj").onclick = async () => {
				await api("POST", `/api/items/${encodeURIComponent(id)}/reject`, {
					feedback: prompt("Feedback (optional)") || "",
				});
				await refreshItem(id);
				close();
			};
		}
	}
	function openCreate() {
		const prOpts = PRIOS.map(
			(p) => `<option ${p === "medium" ? "selected" : ""}>${p}</option>`,
		).join("");
		const stOpts = COLS.map(
			(s) => `<option ${s === "backlog" ? "selected" : ""}>${s}</option>`,
		).join("");
		const dp = defaultProject(items);
		$("#modal-root").innerHTML =
			`<div class="modal"><div class="modal-card"><h2>New item</h2><label>Title</label><input type="text" id="c-title" /><label>Description</label><textarea id="c-desc" rows="3"></textarea><label>Project</label><input type="text" id="c-proj" value="${esc(dp)}" /><label>Priority</label><select id="c-pri">${prOpts}</select><label>Status</label><select id="c-st">${stOpts}</select><label>Labels (comma-separated)</label><input type="text" id="c-lb" placeholder="bug, infra" /><div class="modal-actions"><button type="button" class="btn-ghost" id="c-x">Cancel</button><button type="button" class="btn-primary" id="c-ok">Create</button></div></div></div>`;
		const shut = () => ($("#modal-root").innerHTML = "");
		$("#c-x").onclick = shut;
		$("#c-ok").onclick = async () => {
			const title = $("#c-title").value.trim();
			if (!title) return alert("Title required");
			const labels = $("#c-lb")
				.value.split(",")
				.map((s) => s.trim())
				.filter(Boolean);
			const it = await api("POST", "/api/items", {
				title,
				description: $("#c-desc").value || undefined,
				project: $("#c-proj").value.trim() || undefined,
				priority: $("#c-pri").value,
				status: $("#c-st").value,
				labels: labels.length ? labels : undefined,
			});
			upsertItem(it);
			shut();
			render();
		};
	}
	function render() {
		renderFilters();
		if (view === "board") {
			$("#view-board").classList.remove("is-hidden");
			$("#view-list").classList.add("is-hidden");
			renderBoard();
		} else {
			$("#view-list").classList.remove("is-hidden");
			$("#view-board").classList.add("is-hidden");
			renderList($, filtered, sort, STR, PRN, openDetail);
		}
	}
	function bindHeader() {
		$("#btn-board").onclick = () => {
			view = "board";
			$("#btn-board").classList.add("is-active");
			$("#btn-list").classList.remove("is-active");
			render();
		};
		$("#btn-list").onclick = () => {
			view = "list";
			$("#btn-list").classList.add("is-active");
			$("#btn-board").classList.remove("is-active");
			render();
		};
		$("#btn-create").onclick = () => openCreate();
	}
	document.onkeydown = (e) => {
		if (e.key === "Escape") {
			closeDetail();
			$("#modal-root").innerHTML = "";
		}
	};
	bindHeader();
	loadItems().catch((e) => alert(e.message));
	connectSse((id) => refreshItem(id));
})();
