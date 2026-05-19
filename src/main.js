// Concordance frontend.
// Wires the UI to the Rust engine via Tauri IPC (window.__TAURI__.core.invoke).
// Document parsing (PDF / DOCX / TXT) happens in-browser via locally-bundled PDF.js + mammoth.js (src/vendor/). No network at runtime. Air-gap compatible.

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
// Document parsing (PDF / DOCX / TXT)
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
  el.className = "status" + (cls ? " " + cls : "");
}

function bindFileUpload(btnId, fileId, statusId, onLoaded) {
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

// --------------------------------------------------------------------------
// State
// --------------------------------------------------------------------------
const state = {
  // The wordlist currently loaded into the Check section. Set either by
  // uploading a JSON file, or by clicking "Use extracted wordlist" after Extract.
  checkPreset: null,
  // The most recently extracted preset (from the Extract section).
  extractedPreset: null,
};

// --------------------------------------------------------------------------
// EXTRACT
// --------------------------------------------------------------------------
bindFileUpload("extract-doc-btn", "extract-doc-file", "extract-doc-status", (text) => {
  document.getElementById("extract-text").value = text;
});

// Show/hide language selector based on stop-words checkbox
const stopwordsCheckbox = document.getElementById("extract-stopwords");
const langWrap = document.getElementById("extract-lang-wrap");
function syncLangVisibility() {
  langWrap.style.display = stopwordsCheckbox.checked ? "" : "none";
}
stopwordsCheckbox.addEventListener("change", syncLangVisibility);
syncLangVisibility();

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
  const removeStop = stopwordsCheckbox.checked;
  try {
    const preset = await invoke("extract_preset", {
      text,
      minFrequency: minFreq,
      language: lang,
      removeStopWords: removeStop,
    });
    state.extractedPreset = preset;
    document.getElementById("check-use-extracted").disabled = false;
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
    empty.textContent = "No terms met the minimum frequency. Lower it and try again.";
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
    const count =
      t.source && t.source.startsWith("extracted:")
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
  dlBtn.className = "ghost";
  dlBtn.textContent = "Download as JSON";
  dlBtn.addEventListener("click", () => downloadPreset(preset));
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
// CHECK
// --------------------------------------------------------------------------
document.getElementById("check-preset-btn").addEventListener("click", () =>
  document.getElementById("check-preset-file").click()
);
document.getElementById("check-preset-file").addEventListener("change", async (e) => {
  const status = document.getElementById("check-preset-status");
  const f = e.target.files[0];
  if (!f) return;
  try {
    const text = await f.text();
    const parsed = JSON.parse(text);
    if (!parsed.terms || !Array.isArray(parsed.terms)) {
      throw new Error("Wordlist JSON must have a 'terms' array.");
    }
    state.checkPreset = parsed;
    setStatus(status, `${f.name} (${parsed.terms.length} terms)`, "ok");
  } catch (err) {
    setStatus(status, err.message, "err");
  }
});

document.getElementById("check-use-extracted").addEventListener("click", () => {
  if (!state.extractedPreset) return;
  state.checkPreset = state.extractedPreset;
  setStatus(
    document.getElementById("check-preset-status"),
    `using extracted wordlist (${state.extractedPreset.terms.length} terms)`,
    "ok"
  );
});

bindFileUpload("check-doc-btn", "check-doc-file", "check-doc-status", (text) => {
  document.getElementById("check-text").value = text;
});

document.getElementById("check-run").addEventListener("click", async () => {
  const resultsEl = document.getElementById("check-results");
  resultsEl.innerHTML = "";
  const text = document.getElementById("check-text").value;
  if (!text.trim()) {
    showError(resultsEl, "Paste text or upload a document to check.");
    return;
  }
  if (!state.checkPreset) {
    showError(resultsEl, "Provide a wordlist first (upload JSON or extract one above).");
    return;
  }
  const mode = document.querySelector('input[name="mode"]:checked').value;
  try {
    const result = await invoke("check_text", {
      text,
      presetJson: JSON.stringify(state.checkPreset),
      mode,
    });
    renderCheckResults(resultsEl, result, state.checkPreset);
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
      ? "All wordlist terms appear in your text"
      : `${result.hits.length} wordlist term${result.hits.length === 1 ? "" : "s"} missing`
    : result.clean
      ? "No wordlist terms found"
      : `${result.hits.length} wordlist term${result.hits.length === 1 ? "" : "s"} found`;
  const summary = document.createElement("div");
  summary.className = "result-summary " + (result.clean ? "clean" : "flagged");
  summary.innerHTML = `
    <strong>${escapeHtml(headline)}</strong>
    <span>Wordlist: ${escapeHtml(preset.name || "(unnamed)")} (${result.total_terms_checked} terms). Input: ${result.text_length.toLocaleString()} chars.</span>
  `;
  el.appendChild(summary);

  if (result.hits.length === 0) return;

  const table = document.createElement("table");
  table.innerHTML = `
    <thead>
      <tr>
        <th>${isMissingMode ? "Missing" : "Term"}</th>
        <th>Count</th>
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
      <td class="contexts">${hit.contexts.map((c) => `<span class="ctx">${escapeHtml(c)}</span>`).join("") || "&mdash;"}</td>
    `;
    tbody.appendChild(tr);
  }
  el.appendChild(table);
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
