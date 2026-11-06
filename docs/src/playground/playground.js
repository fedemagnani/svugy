// Turn the overview's d2 textarea into a highlighted editor whose diagram re-renders, through svugy compiled to wasm, as you type.
(() => {
  'use strict';

  const RENDER_DELAY_MS = 250;
  const KEYWORDS = new Set([
    'vars', 'svugy', 'style', 'shape', 'label', 'link', 'direction',
    'caption', 'height', 'position', 'badge', 'subtitle', 'legend',
    'title', 'entries', 'dashes', 'badges', 'x', 'y',
  ]);
  const TOKEN_PATTERN = /("(?:[^"\\\n]|\\.)*")|(#.*)|(<->|->|<-|--)|([A-Za-z_][\w.-]*)(?=\s*:)|(-?\b\d+(?:\.\d+)?\b)|([{}[\];:])/g;

  const source = document.getElementById('svugy-source');
  if (!source) {
    return;
  }
  const encodedWasm = globalThis.svugyWasmBase64;
  const preview = document.getElementById('svugy-preview');
  const diagnosticsList = document.getElementById('svugy-diagnostics');
  const highlight = buildEditor();
  let renderer = null;
  let pendingRender = null;
  let previewUrl = null;

  highlightSource();
  source.addEventListener('input', onSourceEdited);
  source.addEventListener('scroll', syncHighlightScroll);
  diagnosticsList.addEventListener('click', onDiagnosticClicked);
  if (encodedWasm) {
    renderSoon();
  } else {
    showStatus('The live renderer was not built: run `just docs`. Showing the pre-rendered diagram.');
  }

  function buildEditor() {
    const playground = document.createElement('div');
    playground.className = 'svugy-playground';
    const editor = document.createElement('div');
    editor.className = 'svugy-editor';
    const highlightBlock = document.createElement('pre');
    highlightBlock.className = 'svugy-highlight';
    highlightBlock.setAttribute('aria-hidden', 'true');
    const highlightCode = document.createElement('code');
    highlightBlock.appendChild(highlightCode);
    const previewPane = document.createElement('div');
    previewPane.className = 'svugy-preview';
    source.parentNode.insertBefore(playground, source);
    editor.append(highlightBlock, source);
    previewPane.append(preview);
    playground.append(editor, previewPane);
    playground.after(diagnosticsList);
    return highlightCode;
  }

  function onSourceEdited() {
    highlightSource();
    renderSoon();
  }

  function renderSoon() {
    clearTimeout(pendingRender);
    pendingRender = setTimeout(renderSource, RENDER_DELAY_MS);
  }

  function highlightSource() {
    const text = source.value;
    const fragment = document.createDocumentFragment();
    let plainStart = 0;
    for (const match of text.matchAll(TOKEN_PATTERN)) {
      appendText(fragment, text.slice(plainStart, match.index), null);
      appendText(fragment, match[0], tokenClass(match));
      plainStart = match.index + match[0].length;
    }
    appendText(fragment, text.slice(plainStart), null);
    appendText(fragment, ' ', null);
    highlight.replaceChildren(fragment);
    syncHighlightScroll();
  }

  function tokenClass(match) {
    const [, string, comment, arrow, key, number, punctuation] = match;
    if (string) return 'svugy-token-string';
    if (comment) return 'svugy-token-comment';
    if (arrow) return 'svugy-token-arrow';
    if (key) return KEYWORDS.has(key) ? 'svugy-token-keyword' : 'svugy-token-key';
    if (number) return 'svugy-token-number';
    if (punctuation) return 'svugy-token-punctuation';
    return null;
  }

  function appendText(fragment, text, className) {
    if (text === '') return;
    if (className === null) {
      fragment.append(document.createTextNode(text));
      return;
    }
    const span = document.createElement('span');
    span.className = className;
    span.textContent = text;
    fragment.append(span);
  }

  function syncHighlightScroll() {
    const highlightBlock = highlight.parentElement;
    highlightBlock.scrollTop = source.scrollTop;
    highlightBlock.scrollLeft = source.scrollLeft;
  }

  async function renderSource() {
    let verdict;
    try {
      renderer ??= await instantiateRenderer();
      verdict = renderer(source.value);
    } catch (error) {
      renderer = null;
      showStatus(`The renderer crashed on this input: ${error}`);
      return;
    }
    showDiagnostics(verdict.diagnostics);
    if (verdict.svg) {
      showPreview(verdict.svg);
    }
  }

  async function instantiateRenderer() {
    const binary = atob(encodedWasm);
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const { memory, input_buffer, render, output_pointer } = instance.exports;
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    return (text) => {
      const encoded = encoder.encode(text);
      const inputStart = input_buffer(encoded.length);
      new Uint8Array(memory.buffer, inputStart, encoded.length).set(encoded);
      const outputLength = render();
      const output = new Uint8Array(memory.buffer, output_pointer(), outputLength);
      return JSON.parse(decoder.decode(output));
    };
  }

  function showPreview(svg) {
    const blob = new Blob([svg], { type: 'image/svg+xml' });
    const nextUrl = URL.createObjectURL(blob);
    const current = document.getElementById('svugy-preview');
    const next = current.cloneNode(false);
    next.data = nextUrl;
    next.style.visibility = 'hidden';
    next.addEventListener('load', () => {
      next.style.visibility = '';
      current.remove();
      if (previewUrl) URL.revokeObjectURL(previewUrl);
      previewUrl = nextUrl;
    }, { once: true });
    current.after(next);
  }

  function showDiagnostics(diagnostics) {
    const items = diagnostics.map(diagnosticItem);
    const errorCount = diagnostics.filter((diagnostic) => diagnostic.is_error).length;
    if (errorCount > 0) {
      const stale = document.createElement('li');
      stale.className = 'svugy-diagnostic-status';
      stale.textContent = 'Showing the last diagram that rendered.';
      items.push(stale);
    }
    diagnosticsList.replaceChildren(...items);
  }

  function diagnosticItem(diagnostic) {
    const item = document.createElement('li');
    item.className = diagnostic.is_error ? 'svugy-diagnostic-error' : 'svugy-diagnostic-warning';
    const location = diagnostic.line ? `${diagnostic.line}:${diagnostic.column} ` : '';
    const help = diagnostic.help ? ` · ${diagnostic.help}` : '';
    item.textContent = `${location}${diagnostic.message}${help}`;
    if (diagnostic.line) {
      item.dataset.line = diagnostic.line;
      item.dataset.column = diagnostic.column;
    }
    return item;
  }

  function onDiagnosticClicked(event) {
    const item = event.target.closest('li[data-line]');
    if (!item) return;
    const line = Number(item.dataset.line);
    const column = Number(item.dataset.column);
    const lines = source.value.split('\n');
    const lineStart = lines.slice(0, line - 1).reduce((offset, text) => offset + text.length + 1, 0);
    const caret = lineStart + column - 1;
    source.focus();
    source.setSelectionRange(caret, caret);
  }

  function showStatus(text) {
    const status = document.createElement('li');
    status.className = 'svugy-diagnostic-status';
    status.textContent = text;
    diagnosticsList.replaceChildren(status);
  }
})();
