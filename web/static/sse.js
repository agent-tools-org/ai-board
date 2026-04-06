export function connectSse(onItemId) {
	const es = new EventSource("/api/stream");
	es.onmessage = (ev) => {
		try {
			const d = JSON.parse(ev.data);
			if (d.item_id) onItemId(d.item_id);
		} catch {
			/* ignore */
		}
	};
	es.onerror = () => {
		es.close();
		setTimeout(() => connectSse(onItemId), 3000);
	};
}
