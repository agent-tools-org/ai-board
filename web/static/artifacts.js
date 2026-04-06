import { esc } from "./utils.js";

const ICON = {
	design_doc: "📋",
	investigation: "🔍",
	audit_report: "✅",
};

function renderMd(src) {
	return esc(src)
		.replace(/^### (.+)$/gm, "<h4>$1</h4>")
		.replace(/^## (.+)$/gm, "<h3>$1</h3>")
		.replace(/^# (.+)$/gm, "<h2>$1</h2>")
		.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
		.replace(/`([^`]+)`/g, "<code>$1</code>")
		.replace(/^- (.+)$/gm, "<li>$1</li>")
		.replace(/(<li>.*<\/li>)/gs, "<ul>$1</ul>")
		.replace(/\n/g, "<br>");
}

function hasFinal(artifacts, type) {
	return artifacts.some(
		(a) => a.artifact_type === type && a.status === "final",
	);
}

export function gateWarningHtml(status, artifacts) {
	const list = Array.isArray(artifacts) ? artifacts : [];
	if (status === "ready") {
		if (!hasFinal(list, "design_doc") && !hasFinal(list, "investigation")) {
			return `<div class="gate-warning">⚠ Missing: design_doc or investigation (final) — required to move to active</div>`;
		}
	} else if (status === "active") {
		if (!hasFinal(list, "audit_report")) {
			return `<div class="gate-warning">⚠ Missing: audit_report (final) — required to move to review</div>`;
		}
	}
	return "";
}

export function artifactsSectionHtml(artifacts, errMsg) {
	const arts = Array.isArray(artifacts) ? artifacts : [];
	const rows = arts
		.map((af) => {
			const icon = ICON[af.artifact_type] || "📄";
			const stClass = af.status === "final" ? "final" : "draft";
			const aid = esc(af.id);
			const hasContent = af.content && af.content.trim();
			const contentBlock = hasContent
				? `<button type="button" class="artifact-expand" data-af-id="${aid}" aria-label="Toggle content">▶</button><div class="artifact-content is-hidden" data-af-content="${aid}">${renderMd(af.content)}</div>`
				: "";
			return `<div class="artifact">
				<span class="artifact-type" title="${esc(af.artifact_type)}">${icon}</span>
				<span class="artifact-title">${esc(af.title)}</span>
				<span class="artifact-status ${stClass}">${esc(af.status)}</span>
				<span class="artifact-by">${esc(af.created_by || "—")}</span>
				${hasContent ? `<button type="button" class="artifact-expand" data-af-expand="${aid}" aria-label="Toggle content">▶</button>` : ""}
				<button type="button" class="artifact-del" data-af-id="${aid}" aria-label="Delete artifact">×</button>
			</div>${hasContent ? `<div class="artifact-content is-hidden" data-af-content="${aid}">${renderMd(af.content)}</div>` : ""}`;
		})
		.join("");
	const err = errMsg
		? `<div class="artifact-load-err">${esc(errMsg)}</div>`
		: "";
	const list =
		rows ||
		`<p class="artifact-empty">No artifacts yet.</p>`;
	const typeOpts = ["design_doc", "investigation", "audit_report"]
		.map((t) => `<option value="${esc(t)}">${esc(t)}</option>`)
		.join("");
	return `<div class="artifacts">
		<h3>Artifacts</h3>
		${err}
		<div class="artifact-list">${list}</div>
		<button type="button" class="btn-ghost" id="af-toggle">Add artifact</button>
		<div id="af-form" class="is-hidden">
			<div class="panel-row"><label>Type</label><select id="af-type">${typeOpts}</select></div>
			<div class="panel-row"><label>Title</label><input type="text" id="af-title" placeholder="Title" /></div>
			<div class="panel-row"><label>Content</label><textarea id="af-content" rows="4" placeholder="Optional"></textarea></div>
			<div class="panel-row af-status-row"><label>Status</label>
				<label class="af-radio"><input type="radio" name="af-st" value="draft" checked /> draft</label>
				<label class="af-radio"><input type="radio" name="af-st" value="final" /> final</label>
			</div>
			<button type="button" class="btn-primary" id="af-save">Save artifact</button>
		</div>
	</div>`;
}

export function bindArtifactsPanel($, itemId, api, refreshDetail) {
	const toggle = $("#af-toggle");
	const form = $("#af-form");
	if (toggle && form) {
		toggle.onclick = () => form.classList.toggle("is-hidden");
	}
	document.querySelectorAll("[data-af-expand]").forEach((btn) => {
		btn.onclick = () => {
			const content = document.querySelector(`[data-af-content="${btn.dataset.afExpand}"]`);
			if (content) {
				content.classList.toggle("is-hidden");
				btn.textContent = content.classList.contains("is-hidden") ? "▶" : "▼";
			}
		};
	});
	const save = $("#af-save");
	if (save) {
		save.onclick = async () => {
			const title = $("#af-title").value.trim();
			if (!title) {
				alert("Title required");
				return;
			}
			const st =
				document.querySelector('input[name="af-st"]:checked')?.value || "draft";
			try {
				await api("POST", `/api/items/${encodeURIComponent(itemId)}/artifacts`, {
					artifact_type: $("#af-type").value,
					title,
					content: $("#af-content").value || undefined,
					status: st,
				});
				await refreshDetail();
			} catch (e) {
				alert(e.message || String(e));
			}
		};
	}
	const panel = $("#detail-panel");
	if (panel) {
		panel.querySelectorAll(".artifact-del").forEach((btn) => {
			btn.onclick = async (ev) => {
				ev.stopPropagation();
				const afId = btn.dataset.afId;
				if (!afId) return;
				if (!confirm("Delete this artifact?")) return;
				try {
					await api(
						"DELETE",
						`/api/items/${encodeURIComponent(itemId)}/artifacts/${encodeURIComponent(afId)}`,
					);
					await refreshDetail();
				} catch (err) {
					alert(err.message || String(err));
				}
			};
		});
	}
}
