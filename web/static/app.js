(() => {
	const COLS = ["backlog", "ready", "active", "review", "done", "blocked", "rejected"];
	const PRIOS = ["critical", "high", "medium", "low"];
	const PRN = { critical: 0, high: 1, medium: 2, low: 3 };
	const STR = ["id", "title", "status", "assignee"];
	const items = new Map();
	const filters = { st: new Set(), pr: new Set(), lb: new Set() };
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
	function rel(iso) {
		const s = (Date.now() - new Date(iso)) / 1000;
		if (s < 60) return `${Math.max(0, Math.floor(s))}s ago`;
		const m = s / 60;
		if (m < 60) return `${Math.floor(m)}m ago`;
		const h = m / 60;
		return h < 48 ? `${Math.floor(h)}h ago` : `${Math.floor(h / 24)}d ago`;
	}
	function filtered() {
		return [...items.values()].filter((it) => {
			if (filters.st.size && !filters.st.has(it.status)) return false;
			if (filters.pr.size && !filters.pr.has(it.priority)) return false;
			if (filters.lb.size && !(it.labels || []).some((l) => filters.lb.has(l))) return false;
			return true;
		});
	}
	function upsertItem(it) {
		items.set(it.id, it);
	}
	async function loadItems() {
		const list = await api("GET", "/api/items?limit=2000");
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
		const pill = (type, val, lab) => `<button type="button" class="pill ${filters[type].has(val) ? "is-on" : ""}" data-flt="${type}" data-v="${val}">${lab}</button>`;
		const fb = $("#filter-bar");
		fb.innerHTML = `<div class="group"><label>Status</label>${COLS.map((s) => pill("st", s, s)).join("")}</div><div class="group"><label>Priority</label>${PRIOS.map((p) => pill("pr", p, p)).join("")}</div><div class="group"><label>Label</label>${labels.map((l) => pill("lb", l, l)).join("")}</div>`;
		fb.onclick = (e) => {
			const b = e.target.closest("[data-flt]");
			if (!b) return;
			const t = b.dataset.flt;
			const v = b.dataset.v;
			filters[t].has(v) ? filters[t].delete(v) : filters[t].add(v);
			render();
		};
	}
	function esc(t) {
		return String(t).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");
	}
	function cardHtml(it) {
		const pri = it.priority || "medium";
		return `<article class="card pri-${pri}" draggable="true" data-id="${it.id}"><p class="card-title">${esc(it.title)}</p><div class="card-meta"><span class="badge badge-pri-${pri}">${pri}</span>${(it.labels || []).map((l) => `<span class="label-pill">${esc(l)}</span>`).join("")}<span>${it.assignee ? esc(it.assignee) : "—"}</span><span>${rel(it.updated_at)}</span></div></article>`;
	}
	function renderBoard() {
		const list = filtered();
		const by = Object.fromEntries(COLS.map((c) => [c, []]));
		for (const it of list) {
			if (!by[it.status]) by[it.status] = [];
			by[it.status].push(it);
		}
		for (const c of COLS) by[c].sort((a, b) => a.position - b.position);
		$("#view-board").innerHTML = `<div class="board">${COLS.map(
			(st) => `<div class="column" data-status="${st}"><h2>${st}</h2><div class="column-body">${(by[st] || []).map(cardHtml).join("")}</div></div>`,
		).join("")}</div>`;
		bindBoardDnD();
	}
	function listCmp(a, b) {
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
	function renderList() {
		const rows = filtered().sort(listCmp);
		const vl = $("#view-list");
		vl.innerHTML = `<table class="data"><thead><tr><th data-sort="id">ID</th><th data-sort="priority">Priority</th><th data-sort="status">Status</th><th data-sort="title">Title</th><th data-sort="assignee">Assignee</th><th data-sort="updated_at">Updated</th></tr></thead><tbody>${rows
			.map(
				(it) =>
					`<tr data-id="${it.id}"><td>${esc(it.id)}</td><td>${it.priority}</td><td>${it.status}</td><td>${esc(it.title)}</td><td>${it.assignee ? esc(it.assignee) : "—"}</td><td>${rel(it.updated_at)}</td></tr>`,
			)
			.join("")}</tbody></table>`;
		vl.querySelectorAll("th[data-sort]").forEach((th) => {
			th.onclick = () => {
				const k = th.dataset.sort;
				sort.d = sort.k === k ? -sort.d : -1;
				sort.k = k;
				renderList();
			};
		});
		for (const tr of vl.querySelectorAll("tbody tr")) tr.onclick = () => openDetail(tr.dataset.id);
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
					const idx = [...body.querySelectorAll(".card")].filter((c) => c.dataset.id !== id).length;
					try {
						await api("PATCH", "/api/items/reorder", [{ id, position: (idx + 1) * 1000, status: col.dataset.status }]);
						await refreshItem(id);
					} catch (err) {
						alert(err.message);
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
		const evs = data.events || [];
		const review = it.status === "review";
		const pr = PRIOS.map((p) => `<option ${p === it.priority ? "selected" : ""}>${p}</option>`).join("");
		const st = COLS.map((s) => `<option ${s === it.status ? "selected" : ""}>${s}</option>`).join("");
		const hist = evs
			.slice()
			.reverse()
			.map((ev) => `<div class="event"><strong>${esc(ev.action)}</strong> · ${esc(ev.actor)} · ${rel(ev.created_at)}${ev.detail ? ` — ${esc(ev.detail)}` : ""}</div>`)
			.join("");
		const rev = review ? `<button type="button" class="btn-primary" id="f-ap">Approve</button><button type="button" class="btn-ghost" id="f-rj">Reject</button>` : "";
		$("#panel-root").innerHTML =
			`<div class="overlay-backdrop" id="db"></div><aside class="side-panel" id="detail-panel" data-id="${it.id}"><div class="panel-head"><div><strong>${esc(it.id)}</strong></div><button type="button" id="px" aria-label="Close">×</button></div><div class="panel-body"><div class="panel-row"><label>Title</label><input type="text" id="f-title" value="${esc(it.title)}" /></div><div class="panel-row"><label>Description</label><textarea id="f-desc">${esc(it.description || "")}</textarea></div><div class="panel-row"><label>Priority</label><select id="f-pri">${pr}</select></div><div class="panel-row"><label>Status</label><select id="f-st">${st}</select></div><div class="panel-actions"><button type="button" class="btn-primary" id="f-save">Save</button>${rev}<button type="button" class="btn-danger" id="f-del">Delete</button></div><div class="events"><h3>History</h3>${hist}</div></div></aside>`;
		const close = () => closeDetail();
		$("#db").onclick = close;
		$("#px").onclick = close;
		$("#f-save").onclick = async () => {
			await api("PATCH", `/api/items/${encodeURIComponent(id)}`, {
				title: $("#f-title").value,
				description: $("#f-desc").value,
				priority: $("#f-pri").value,
				status: $("#f-st").value,
			});
			await refreshItem(id);
			close();
		};
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
		const prOpts = PRIOS.map((p) => `<option ${p === "medium" ? "selected" : ""}>${p}</option>`).join("");
		const stOpts = COLS.map((s) => `<option ${s === "backlog" ? "selected" : ""}>${s}</option>`).join("");
		$("#modal-root").innerHTML =
			`<div class="modal"><div class="modal-card"><h2>New item</h2><label>Title</label><input type="text" id="c-title" /><label>Description</label><textarea id="c-desc" rows="3"></textarea><label>Priority</label><select id="c-pri">${prOpts}</select><label>Status</label><select id="c-st">${stOpts}</select><label>Labels (comma-separated)</label><input type="text" id="c-lb" placeholder="bug, infra" /><div class="modal-actions"><button type="button" class="btn-ghost" id="c-x">Cancel</button><button type="button" class="btn-primary" id="c-ok">Create</button></div></div></div>`;
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
			renderList();
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
	function connectSse() {
		const es = new EventSource("/api/stream");
		es.onmessage = (ev) => {
			try {
				const d = JSON.parse(ev.data);
				if (d.item_id) refreshItem(d.item_id);
			} catch {
				/* ignore */
			}
		};
		es.onerror = () => {
			es.close();
			setTimeout(connectSse, 3000);
		};
	}
	bindHeader();
	loadItems().catch((e) => alert(e.message));
	connectSse();
})();
