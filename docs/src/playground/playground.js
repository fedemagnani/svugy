// Turn every `.svugy-live` container on a page into a highlighted editor whose diagram re-renders, through svugy compiled to wasm, as you type.
(() => {
  'use strict';

  const RENDER_DELAY_MS = 250;
  const LANGUAGES = {
    markdown: {
      label: 'Markdown',
      byte: 0,
      tokenPattern: /(?<comment><!--[\s\S]*?-->)|(?<string>"(?:[^"\\\n]|\\.)*")|(?<keyword>^---[ \t]*$|^#{1,6}[ \t].*$)|(?<arrow>->)|(?<key>[A-Za-z_][\w.-]*(?==)|^[ \t]*[A-Za-z_][\w-]*(?=:))|(?<number>#[0-9A-Fa-f]{3,8}\b|-?\b\d+(?:\.\d+)?\b)|(?<punctuation>^[ \t]*-(?=[ \t])|[(),=:])/gm,
    },
    d2: {
      label: 'd2',
      byte: 1,
      tokenPattern: /(?<string>"(?:[^"\\\n]|\\.)*")|(?<comment>#.*)|(?<arrow><->|->|<-|--)|(?<keyword>(?<![\w.-])(?:vars|svugy|style|shape|label|link|direction|caption|height|position|badge|subtitle|legend|title|entries|dashes|badges|x|y)(?![\w.-])(?=\s*:))|(?<key>[A-Za-z_][\w.-]*(?=\s*:))|(?<number>-?\b\d+(?:\.\d+)?\b)|(?<punctuation>[{}[\];:])/g,
    },
  };

  const encodedWasm = globalThis.svugyWasmBase64;
  let sharedRendererPromise = null;

  const unmountedContainers = document.querySelectorAll('.svugy-live:not([data-svugy-mounted])');
  unmountedContainers.forEach(mountLiveEditor);

  // Mount one editor: its first `textarea[data-svugy-language]` is shown, and any further one turns into a switchable draft.
  function mountLiveEditor(container, editorIndex) {
    container.dataset.svugyMounted = '';
    const sources = container.querySelectorAll('textarea[data-svugy-language]');
    const [source, ...alternateSources] = sources;
    const drafts = {};
    for (const draftSource of sources) {
      const draftLanguageName = draftSource.dataset.svugyLanguage;
      drafts[draftLanguageName] = draftSource.value;
    }
    const isSwitchable = alternateSources.length > 0;
    let activeLanguageName = source.dataset.svugyLanguage;
    let shownPreview = container.querySelector('object.svugy');
    let loadingPreview = null;
    let pendingRender = null;
    const diagnosticsList = document.createElement('ul');
    diagnosticsList.className = 'svugy-diagnostics';
    const highlight = buildEditor();

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
      source.spellcheck = false;
      source.setAttribute('autocapitalize', 'off');
      source.setAttribute('autocomplete', 'off');
      labelSource();
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
      editor.append(highlightBlock, source);
      previewPane.append(shownPreview);
      playground.append(editor, previewPane);
      const editorParts = [playground, diagnosticsList];
      if (isSwitchable) {
        const languageSwitch = buildLanguageSwitch();
        editorParts.unshift(languageSwitch);
      }
      container.replaceChildren(...editorParts);
      return highlightCode;
    }

    function labelSource() {
      const activeLanguage = LANGUAGES[activeLanguageName];
      source.setAttribute('aria-label', `${activeLanguage.label} source`);
    }

    function buildLanguageSwitch() {
      const languageSwitch = document.createElement('div');
      languageSwitch.className = 'svugy-language-switch';
      languageSwitch.setAttribute('role', 'radiogroup');
      languageSwitch.setAttribute('aria-label', 'Source language');
      for (const languageName of Object.keys(drafts)) {
        const language = LANGUAGES[languageName];
        const option = document.createElement('label');
        const radio = document.createElement('input');
        radio.type = 'radio';
        radio.name = `svugy-language-${editorIndex}`;
        radio.value = languageName;
        radio.checked = languageName === activeLanguageName;
        option.append(radio, ` ${language.label}`);
        languageSwitch.append(option);
      }
      languageSwitch.addEventListener('change', onLanguageSwitched);
      return languageSwitch;
    }

    function onLanguageSwitched(event) {
      drafts[activeLanguageName] = source.value;
      activeLanguageName = event.target.value;
      source.value = drafts[activeLanguageName];
      labelSource();
      onSourceEdited();
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
      const { tokenPattern } = LANGUAGES[activeLanguageName];
      const highlighted = highlightedFragment(source.value, tokenPattern);
      highlight.replaceChildren(highlighted);
      syncHighlightScroll();
    }

    function syncHighlightScroll() {
      const highlightBlock = highlight.parentElement;
      highlightBlock.scrollTop = source.scrollTop;
      highlightBlock.scrollLeft = source.scrollLeft;
    }

    async function renderSource() {
      let verdict;
      try {
        sharedRendererPromise ??= instantiateRenderer();
        const renderer = await sharedRendererPromise;
        const { byte } = LANGUAGES[activeLanguageName];
        verdict = renderer(source.value, byte);
      } catch (error) {
        sharedRendererPromise = null;
        showStatus(`The renderer crashed on this input: ${error}`);
        return;
      }
      showDiagnostics(verdict.diagnostics);
      if (verdict.svg) {
        showPreview(verdict.svg);
      }
    }

    function showPreview(svg) {
      const blob = new Blob([svg], { type: 'image/svg+xml' });
      const nextUrl = URL.createObjectURL(blob);
      const next = shownPreview.cloneNode(false);
      next.data = nextUrl;
      next.style.visibility = 'hidden';
      discardLoadingPreview();
      loadingPreview = next;
      next.addEventListener('load', () => {
        next.style.visibility = '';
        const replacedUrl = shownPreview.data;
        shownPreview.remove();
        if (replacedUrl.startsWith('blob:')) URL.revokeObjectURL(replacedUrl);
        shownPreview = next;
        loadingPreview = null;
      }, { once: true });
      shownPreview.after(next);
    }

    function discardLoadingPreview() {
      if (!loadingPreview) return;
      const discardedUrl = loadingPreview.data;
      loadingPreview.remove();
      URL.revokeObjectURL(discardedUrl);
      loadingPreview = null;
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

    function onDiagnosticClicked(event) {
      const item = event.target.closest('li[data-line]');
      if (!item) return;
      const line = Number(item.dataset.line);
      const column = Number(item.dataset.column);
      const lines = source.value.split('\n');
      const linesBefore = lines.slice(0, line - 1);
      const lineStart = linesBefore.reduce((offset, text) => offset + text.length + 1, 0);
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
  }

  function highlightedFragment(text, tokenPattern) {
    const fragment = document.createDocumentFragment();
    let plainStart = 0;
    for (const match of text.matchAll(tokenPattern)) {
      const plainText = text.slice(plainStart, match.index);
      appendText(fragment, plainText, null);
      const matchClass = tokenClass(match);
      appendText(fragment, match[0], matchClass);
      plainStart = match.index + match[0].length;
    }
    const trailingText = text.slice(plainStart);
    appendText(fragment, trailingText, null);
    appendText(fragment, ' ', null);
    return fragment;
  }

  function tokenClass(match) {
    const tokenGroups = Object.entries(match.groups);
    const [tokenKind] = tokenGroups.find(([, text]) => text !== undefined);
    return `svugy-token-${tokenKind}`;
  }

  function appendText(fragment, text, className) {
    if (text === '') return;
    if (className === null) {
      const textNode = document.createTextNode(text);
      fragment.append(textNode);
      return;
    }
    const span = document.createElement('span');
    span.className = className;
    span.textContent = text;
    fragment.append(span);
  }

  async function instantiateRenderer() {
    const binary = atob(encodedWasm);
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const { memory, input_buffer, render, output_pointer } = instance.exports;
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    return (text, languageByte) => {
      const encoded = encoder.encode(text);
      const inputStart = input_buffer(encoded.length);
      const inputView = new Uint8Array(memory.buffer, inputStart, encoded.length);
      inputView.set(encoded);
      const outputLength = render(languageByte);
      const outputStart = output_pointer();
      const output = new Uint8Array(memory.buffer, outputStart, outputLength);
      const verdictJson = decoder.decode(output);
      return JSON.parse(verdictJson);
    };
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
})();
