// Concordance frontend.
// Wires the UI to the Rust engine via Tauri IPC (window.__TAURI__.core.invoke).
// Document parsing (PDF / DOCX / TXT) happens in-browser via PDF.js + mammoth.js
// so the Rust engine stays plain-text-in / structured-result-out.

const invoke = (...args) =>
  (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke
    ? window.__TAURI__.core.invoke
    : async () => {
        throw new Error("Tauri IPC unavailable (running outside Tauri shell)");
      })(...args);

// --------------------------------------------------------------------------
// Tab switching
// --------------------------------------------------------------------------
document.querySelectorAll(".tab").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((b) => {
      b.classList.toggle("active", b === btn);
      b.setAttribute("aria-selected", b === btn ? "true" : "false");
    });
    const target = btn.dataset.tab;
    document.querySelectorAll(".tab-panel").forEach((p) => {
      const match = p.id === `tab-${target}`;
      p.classList.toggle("active", match);
      p.hidden = !match;
    });
  });
});

// --------------------------------------------------------------------------
// Document parsing helpers (PDF / DOCX / TXT in-browser)
// --------------------------------------------------------------------------
async function extractTextFromFile(file) {
  const name = file.name.toLowerCase();
  if (name.endsWith(".txt") || name.endsWith(".md")) {
    return await file.text();
  }
  if (name.endsWith(".docx")) {
    if (!window.mammoth) throw new Error("DOCX parser not loaded.");
    const buf = await file.arrayBuffer();
    const result = await window.mammoth.extractRawText({ arrayBuffer: buf });
    return result.value || "";
  }
  if (name.endsWith(".pdf")) {
    if (!window.pdfjsLib) throw new Error("PDF parser not loaded.");
    const buf = await file.arrayBuffer();
    const pdf = await window.pdfjsLib.getDocument({ data: buf }).promise;
    const out = [];
    for (let i = 1; i <= pdf.numPages; i++) {
      const page = await pdf.getPage(i);
      const tc = await page.getTextContent();
      out.push(tc.items.map((it) => it.str).join(" "));
    }
    return out.join("\n\n");
  }
  throw new Error(`Unsupported file type: ${file.name}`);
}

function setStatus(el, message, cls = "") {
  el.textContent = message;
  el.className = "file-status" + (cls ? " " + cls : "");
}

// --------------------------------------------------------------------------
// File picker bindings
// --------------------------------------------------------------------------
function bindFilePicker(btnId, fileId, statusId, onLoaded) {
  const btn = document.getElementById(btnId);
  const file = document.getElementById(fileId);
  const status = document.getElementById(statusId);
  btn.addEventListener("click", () => file.click());
  file.addEventListener("change", async () => {
    if (!file.files[0]) return;
    setStatus(status, `Reading ${file.files[0].name}...`);
    try {
      const text = await extractTextFromFile(file.files[0]);
      setStatus(status, `${file.files[0].name} (${text.length.toLocaleString()} chars)`, "ok");
      onLoaded(text, file.files[0].name);
    } catch (e) {
      setStatus(status, e.message || String(e), "err");
    }
  });
}

// State holders for the Check tab's wordlist source
const presetState = {
  bundledNsf: null,
  bundledAura: null,
  custom: null,
  extracted: null,
};

// Custom JSON preset upload
document.getElementById("custom-preset-btn").addEventListener("click", () =>
  document.getElementById("custom-preset-file").click()
);
document.getElementById("custom-preset-file").addEventListener("change", async (e) => {
  const status = document.getElementById("custom-preset-status");
  const f = e.target.files[0];
  if (!f) return;
  try {
    const text = await f.text();
    const parsed = JSON.parse(text);
    if (!parsed.terms || !Array.isArray(parsed.terms)) {
      throw new Error("Preset JSON must have a 'terms' array.");
    }
    presetState.custom = parsed;
    setStatus(status, `${f.name} (${parsed.terms.length} terms)`, "ok");
    document.querySelector('input[name="preset"][value="custom"]').checked = true;
  } catch (err) {
    setStatus(status, err.message, "err");
  }
});

// Extract-from-document preset upload
bindFilePicker("extract-source-btn", "extract-source-file", "extract-source-status", async (text, name) => {
  const status = document.getElementById("extract-source-status");
  setStatus(status, `Extracting preset from ${name}...`);
  try {
    const minFreq = parseInt(document.getElementById("extract-min-freq").value, 10) || 2;
    const lang = document.getElementById("extract-lang").value || "english";
    const preset = await invoke("extract_preset", { text, minFrequency: minFreq, language: lang });
    presetState.extracted = preset;
    setStatus(status, `${name} -> ${preset.terms.length} terms`, "ok");
    document.querySelector('input[name="preset"][value="extracted"]').checked = true;
  } catch (e) {
    setStatus(status, e.message || String(e), "err");
  }
});

// Check tab's text upload
bindFilePicker("check-doc-btn", "check-doc-file", "check-doc-status", (text) => {
  document.getElementById("check-text").value = text;
});

// Extract tab's text upload
bindFilePicker("extract-doc-btn", "extract-doc-file", "extract-doc-status", (text) => {
  document.getElementById("extract-text").value = text;
});

// --------------------------------------------------------------------------
// Preset loading
// --------------------------------------------------------------------------
async function getActivePreset() {
  const choice = document.querySelector('input[name="preset"]:checked').value;
  if (choice === "bundled-nsf") {
    if (!presetState.bundledNsf) {
      presetState.bundledNsf = await invoke("load_bundled_preset", { name: "nsf-2025-leaked" });
    }
    return presetState.bundledNsf;
  }
  if (choice === "bundled-aura") {
    if (!presetState.bundledAura) {
      presetState.bundledAura = await invoke("load_bundled_preset", { name: "aura-lab-extended" });
    }
    return presetState.bundledAura;
  }
  if (choice === "custom") {
    if (!presetState.custom) throw new Error("Upload a custom preset JSON first.");
    return presetState.custom;
  }
  if (choice === "extracted") {
    if (!presetState.extracted) throw new Error("Upload a document to extract a preset from first.");
    return presetState.extracted;
  }
  throw new Error("No preset selected.");
}

// --------------------------------------------------------------------------
// CHECK mode
// --------------------------------------------------------------------------
document.getElementById("check-run").addEventListener("click", async () => {
  const resultsEl = document.getElementById("check-results");
  resultsEl.innerHTML = "";
  const text = document.getElementById("check-text").value;
  if (!text.trim()) {
    showError(resultsEl, "Paste text or upload a document to check.");
    return;
  }
  const mode = document.querySelector('input[name="mode"]:checked').value;
  let preset;
  try {
    preset = await getActivePreset();
  } catch (e) {
    showError(resultsEl, e.message);
    return;
  }
  try {
    const result = await invoke("check_text", {
      text,
      presetJson: JSON.stringify(preset),
      mode,
    });
    renderCheckResults(resultsEl, result, preset);
  } catch (e) {
    showError(resultsEl, `Engine error: ${e}`);
  }
});

document.getElementById("check-clear").addEventListener("click", () => {
  document.getElementById("check-text").value = "";
  document.getElementById("check-results").innerHTML = "";
});

function renderCheckResults(el, result, preset) {
  const isMissingMode = result.mode === "flag-if-missing";
  const headline = isMissingMode
    ? result.clean
      ? "All preset terms appear in your text"
      : `${result.hits.length} preset term${result.hits.length === 1 ? "" : "s"} missing from your text`
    : result.clean
      ? "No flagged terms found"
      : `${result.hits.length} flagged term${result.hits.length === 1 ? "" : "s"} found`;
  const summary = document.createElement("div");
  summary.className = "result-summary " + (result.clean ? "clean" : "flagged");
  summary.innerHTML = `
    <strong>${escapeHtml(headline)}</strong>
    <span>Preset: ${escapeHtml(preset.name || "(unnamed)")} (${result.total_terms_checked} terms). Input: ${result.text_length.toLocaleString()} chars. Mode: ${escapeHtml(result.mode)}.</span>
  `;
  el.appendChild(summary);

  if (result.hits.length === 0) return;

  const table = document.createElement("table");
  table.innerHTML = `
    <thead>
      <tr>
        <th>${isMissingMode ? "Missing term" : "Term"}</th>
        <th>Count</th>
        <th>Source</th>
        <th>Context</th>
      </tr>
    </thead>
    <tbody></tbody>
  `;
  const tbody = table.querySelector("tbody");
  for (const hit of result.hits) {
    const tr = document.createElement("tr");
    tr.innerHTML = `
      <td class="term">${escapeHtml(hit.term)}</td>
      <td class="count">${hit.count}</td>
      <td class="source">${escapeHtml(hit.source || "")}</td>
      <td class="contexts">${hit.contexts.map((c) => `<span class="ctx">${escapeHtml(c)}</span>`).join("") || "&mdash;"}</td>
    `;
    tbody.appendChild(tr);
  }
  el.appendChild(table);
}

// --------------------------------------------------------------------------
// EXTRACT mode
// --------------------------------------------------------------------------
document.getElementById("extract-run").addEventListener("click", async () => {
  const resultsEl = document.getElementById("extract-results");
  resultsEl.innerHTML = "";
  const text = document.getElementById("extract-text").value;
  if (!text.trim()) {
    showError(resultsEl, "Paste text or upload a document to extract from.");
    return;
  }
  const minFreq = parseInt(document.getElementById("extract-min-freq").value, 10) || 2;
  const lang = document.getElementById("extract-lang").value || "english";
  try {
    const preset = await invoke("extract_preset", {
      text,
      minFrequency: minFreq,
      language: lang,
    });
    renderExtractResults(resultsEl, preset, text.length);
  } catch (e) {
    showError(resultsEl, `Engine error: ${e}`);
  }
});

document.getElementById("extract-clear").addEventListener("click", () => {
  document.getElementById("extract-text").value = "";
  document.getElementById("extract-results").innerHTML = "";
});

function renderExtractResults(el, preset, sourceLen) {
  const summary = document.createElement("div");
  summary.className = "result-summary";
  summary.innerHTML = `
    <strong>${preset.terms.length} terms extracted</strong>
    <span>Source: ${sourceLen.toLocaleString()} chars. ${escapeHtml(preset.description || "")}</span>
  `;
  el.appendChild(summary);

  if (preset.terms.length === 0) {
    const empty = document.createElement("p");
    empty.className = "subtitle";
    empty.textContent = "No terms met the minimum frequency. Try lowering it.";
    el.appendChild(empty);
    return;
  }

  const table = document.createElement("table");
  table.innerHTML = `
    <thead><tr><th>Term</th><th>Count</th></tr></thead>
    <tbody></tbody>
  `;
  const tbody = table.querySelector("tbody");
  for (const t of preset.terms) {
    const count = t.source && t.source.startsWith("extracted:")
      ? parseInt(t.source.slice("extracted:".length), 10) || 0
      : 0;
    const tr = document.createElement("tr");
    tr.innerHTML = `
      <td class="term">${escapeHtml(t.term)}</td>
      <td class="count">${count}</td>
    `;
    tbody.appendChild(tr);
  }
  el.appendChild(table);

  const downloadRow = document.createElement("div");
  downloadRow.className = "download-row";
  const dlBtn = document.createElement("button");
  dlBtn.type = "button";
  dlBtn.textContent = "Download as JSON preset";
  dlBtn.addEventListener("click", () => downloadPreset(preset));
  const useBtn = document.createElement("button");
  useBtn.type = "button";
  useBtn.textContent = "Use this as the Check preset";
  useBtn.style.marginLeft = "0.5rem";
  useBtn.addEventListener("click", () => {
    presetState.extracted = preset;
    document.querySelector('input[name="preset"][value="extracted"]').checked = true;
    document.getElementById("extract-source-status").textContent =
      `(in-memory) ${preset.terms.length} terms`;
    document.getElementById("extract-source-status").className = "file-status ok";
    document.querySelector('.tab[data-tab="check"]').click();
  });
  downloadRow.appendChild(useBtn);
  downloadRow.appendChild(dlBtn);
  el.appendChild(downloadRow);
}

function downloadPreset(preset) {
  const blob = new Blob([JSON.stringify(preset, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `concordance-preset-${Date.now()}.json`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// --------------------------------------------------------------------------
// Utilities
// --------------------------------------------------------------------------
function showError(el, message) {
  const box = document.createElement("div");
  box.className = "error-box";
  box.textContent = message;
  el.appendChild(box);
}

function escapeHtml(s) {
  if (s == null) return "";
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

console.log("Concordance UI ready.");
