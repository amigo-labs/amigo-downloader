// REST + WebSocket client for amigo-downloader API

const API_BASE = "/api/v1";

export async function getStatus() {
  const res = await fetch(`${API_BASE}/status`);
  return res.json();
}

export async function getDownloads() {
  const res = await fetch(`${API_BASE}/downloads`);
  return res.json();
}

export async function addDownload(url: string) {
  const res = await fetch(`${API_BASE}/downloads`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ url }),
  });
  return res.json();
}

export async function importDlc(file: File) {
  const formData = new FormData();
  formData.append("file", file);
  const res = await fetch(`${API_BASE}/downloads/container`, {
    method: "POST",
    body: formData,
  });
  return res.json();
}

export async function exportDlc(ids?: string[]) {
  const params = ids ? `?ids=${ids.join(",")}` : "";
  const res = await fetch(`${API_BASE}/downloads/export/dlc${params}`);
  return res.blob();
}
