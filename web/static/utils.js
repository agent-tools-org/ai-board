export function esc(t) {
	return String(t)
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;")
		.replace(/"/g, "&quot;")
		.replace(/'/g, "&#39;");
}

export function rel(iso) {
	const s = (Date.now() - new Date(iso)) / 1000;
	if (s < 60) return `${Math.max(0, Math.floor(s))}s ago`;
	const m = s / 60;
	if (m < 60) return `${Math.floor(m)}m ago`;
	const h = m / 60;
	return h < 48 ? `${Math.floor(h)}h ago` : `${Math.floor(h / 24)}d ago`;
}

export function defaultProject(items) {
	const m = new Map();
	for (const it of items.values()) {
		const p = it.project;
		if (!p) continue;
		m.set(p, (m.get(p) || 0) + 1);
	}
	let best = "",
		n = 0;
	for (const [k, v] of m)
		if (v > n) {
			n = v;
			best = k;
		}
	return best;
}

export function multiProj(items) {
	const s = new Set();
	for (const it of items.values()) if (it.project) s.add(it.project);
	return s.size > 1;
}
