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
      tokenPattern: /(?<string>"(?:[^"\\\n]|\\.)*")|(?<comment>#.*)|(?<arrow><->|->|<-|--)|(?<keyword>(?<![\w.-])(?:vars|svugy|style|shape|label|link|direction|caption|height|position|badge|subtitle|x|y)(?![\w.-])(?=\s*:))|(?<key>[A-Za-z_][\w.-]*(?=\s*:))|(?<number>-?\b\d+(?:\.\d+)?\b)|(?<punctuation>[{}[\];:])/g,
    },
  };
  const THEMES = {
    standard: { label: 'Standard', byte: 0 },
    sketch: { label: 'Sketch', byte: 1 },
  };

  const encodedWasm = globalThis.svugyWasmBase64;
  let sharedRendererPromise = null;

  const unmountedContainers = document.querySelectorAll('.svugy-live:not([data-svugy-mounted])');
  unmountedContainers.forEach(mountLiveEditor);

  // Mount one editor: its first `textarea[data-svugy-language]` is shown, any further one turns into a switchable draft, a theme switch picks standard or sketch (starting from data-svugy-theme, announcing each pick as a bubbling `svugy-theme` event), and data-svugy-curtain lays the source over the diagram as a curtain.
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
    const requestedThemeName = container.dataset.svugyTheme;
    const isKnownTheme = Object.hasOwn(THEMES, requestedThemeName);
    let activeThemeName = isKnownTheme ? requestedThemeName : 'standard';
    let shownPreview = container.querySelector('object.svugy');
    let loadingPreview = null;
    let pendingRender = null;
    let renderGeneration = 0;
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
      const switches = buildSwitches();
      container.replaceChildren(switches, playground, diagnosticsList);
      const hasCurtain = container.hasAttribute('data-svugy-curtain');
      if (hasCurtain) {
        const curtain = buildCurtain(playground);
        editor.after(curtain);
        announceCurtain(playground, curtain);
      }
      return highlightCode;
    }

    function labelSource() {
      const activeLanguage = LANGUAGES[activeLanguageName];
      source.setAttribute('aria-label', `${activeLanguage.label} source`);
    }

    function buildSwitches() {
      const switches = document.createElement('div');
      switches.className = 'svugy-switches';
      if (isSwitchable) {
        const draftLanguageNames = Object.keys(drafts);
        const languageSwitch = buildRadioSwitch('language', 'Source language', draftLanguageNames, LANGUAGES, activeLanguageName);
        languageSwitch.addEventListener('change', onLanguageSwitched);
        switches.append(languageSwitch);
      }
      const themeNames = Object.keys(THEMES);
      const themeSwitch = buildRadioSwitch('theme', 'Theme', themeNames, THEMES, activeThemeName);
      themeSwitch.addEventListener('change', onThemeSwitched);
      switches.append(themeSwitch);
      return switches;
    }

    function buildRadioSwitch(kind, ariaLabel, choiceNames, choices, checkedName) {
      const radioSwitch = document.createElement('div');
      radioSwitch.className = `svugy-${kind}-switch`;
      radioSwitch.setAttribute('role', 'radiogroup');
      radioSwitch.setAttribute('aria-label', ariaLabel);
      for (const choiceName of choiceNames) {
        const choice = choices[choiceName];
        const option = document.createElement('label');
        const radio = document.createElement('input');
        radio.type = 'radio';
        radio.name = `svugy-${kind}-${editorIndex}`;
        radio.value = choiceName;
        radio.checked = choiceName === checkedName;
        option.append(radio, ` ${choice.label}`);
        radioSwitch.append(option);
      }
      return radioSwitch;
    }

    function onLanguageSwitched(event) {
      drafts[activeLanguageName] = source.value;
      activeLanguageName = event.target.value;
      source.value = drafts[activeLanguageName];
      labelSource();
      onSourceEdited();
    }

    function onThemeSwitched(event) {
      activeThemeName = event.target.value;
      const themeEvent = new CustomEvent('svugy-theme', { bubbles: true, detail: { themeName: activeThemeName } });
      container.dispatchEvent(themeEvent);
      renderSoon();
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

    // A quick draft in the seed arrangement shows first, then the searched layout replaces it; a newer edit discards both.
    async function renderSource() {
      renderGeneration += 1;
      const generation = renderGeneration;
      const languageByte = LANGUAGES[activeLanguageName].byte;
      const themeByte = THEMES[activeThemeName].byte;
      const request = { editorKey: editorIndex, generation, text: source.value, languageByte, themeByte };
      let draftVerdict;
      let fullVerdict;
      try {
        sharedRendererPromise ??= startRenderer();
        const renderer = await sharedRendererPromise;
        const draftPromise = renderer.render({ ...request, isDraft: true });
        const fullPromise = renderer.render({ ...request, isDraft: false });
        draftVerdict = await draftPromise;
        if (generation !== renderGeneration || !draftVerdict) return;
        showVerdict(draftVerdict);
        fullVerdict = await fullPromise;
      } catch (error) {
        discardRenderer();
        showStatus(`The renderer crashed on this input: ${error}`);
        return;
      }
      if (generation !== renderGeneration || !fullVerdict) return;
      const isSameAsDraft = fullVerdict.svg === draftVerdict.svg;
      if (isSameAsDraft) return;
      showVerdict(fullVerdict);
    }

    function showVerdict(verdict) {
      showDiagnostics(verdict.diagnostics);
      if (verdict.svg) {
        showPreview(verdict.svg);
      }
    }

    function showPreview(svg) {
      const blob = new Blob([svg], { type: 'image/svg+xml' });
      const nextUrl = URL.createObjectURL(blob);
      const next = shownPreview.cloneNode(false);
      const shownLevelLink = levelLinkOf(shownPreview);
      next.data = `${nextUrl}${shownLevelLink}`;
      next.dataset.blobUrl = nextUrl;
      next.style.visibility = 'hidden';
      discardLoadingPreview();
      loadingPreview = next;
      next.addEventListener('load', () => {
        next.style.visibility = '';
        const replacedBlobUrl = shownPreview.dataset.blobUrl;
        shownPreview.remove();
        if (replacedBlobUrl) URL.revokeObjectURL(replacedBlobUrl);
        shownPreview = next;
        loadingPreview = null;
      }, { once: true });
      shownPreview.after(next);
    }

    // The level link reopening the reader's shown workflow, focused level and shown layer; the root's frames the first layer, not the whole canvas.
    function levelLinkOf(preview) {
      const enteredLevel = preview.contentDocument?.querySelector('[data-svugy-entered]');
      if (!enteredLevel) return '#svugy=';
      const levelPath = enteredLevel.getAttribute('data-svugy-level');
      const encodedLevelPath = encodeURIComponent(levelPath);
      const shownLayer = enteredLevel.closest('[data-svugy-layer]');
      const layerNumber = shownLayer?.getAttribute('data-svugy-layer');
      const layerParameter = shownLayer ? `&layer=${layerNumber}` : '';
      const shownWorkflow = enteredLevel.closest('[data-svugy-workflow]');
      const workflowId = shownWorkflow?.getAttribute('data-svugy-workflow');
      const encodedWorkflowId = shownWorkflow ? encodeURIComponent(workflowId) : '';
      const workflowParameter = shownWorkflow ? `&workflow=${encodedWorkflowId}` : '';
      return `#svugy=${encodedLevelPath}${layerParameter}${workflowParameter}`;
    }

    function discardLoadingPreview() {
      if (!loadingPreview) return;
      const discardedBlobUrl = loadingPreview.dataset.blobUrl;
      loadingPreview.remove();
      URL.revokeObjectURL(discardedBlobUrl);
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

  // Build the handle that moves the curtain between source and diagram, from all diagram (0) to all source (100).
  function buildCurtain(playground) {
    const curtain = document.createElement('div');
    curtain.className = 'svugy-curtain';
    curtain.tabIndex = 0;
    curtain.setAttribute('role', 'separator');
    curtain.setAttribute('aria-orientation', 'vertical');
    curtain.setAttribute('aria-label', 'Source width');
    curtain.setAttribute('aria-valuemin', '0');
    curtain.setAttribute('aria-valuemax', '100');
    curtain.addEventListener('pointerdown', (event) => {
      curtain.setPointerCapture(event.pointerId);
      playground.dataset.svugyCurtainMoving = '';
    });
    curtain.addEventListener('pointermove', (event) => {
      const isDragged = curtain.hasPointerCapture(event.pointerId);
      if (!isDragged) return;
      const bounds = playground.getBoundingClientRect();
      const sourcePercent = ((event.clientX - bounds.left) / bounds.width) * 100;
      moveCurtain(playground, curtain, sourcePercent);
    });
    curtain.addEventListener('lostpointercapture', () => {
      delete playground.dataset.svugyCurtainMoving;
    });
    curtain.addEventListener('keydown', (event) => {
      const shownPercent = sourcePercentOf(playground);
      const keyedPercents = { ArrowLeft: shownPercent - 5, ArrowRight: shownPercent + 5, Home: 0, End: 100 };
      const keyedPercent = keyedPercents[event.key];
      if (keyedPercent === undefined) return;
      event.preventDefault();
      moveCurtain(playground, curtain, keyedPercent);
    });
    curtain.addEventListener('dblclick', () => {
      playground.style.removeProperty('--svugy-curtain');
      announceCurtain(playground, curtain);
    });
    return curtain;
  }

  function moveCurtain(playground, curtain, sourcePercent) {
    const clampedPercent = Math.min(100, Math.max(0, sourcePercent));
    playground.style.setProperty('--svugy-curtain', `${clampedPercent}%`);
    announceCurtain(playground, curtain);
  }

  function announceCurtain(playground, curtain) {
    const shownPercent = sourcePercentOf(playground);
    curtain.setAttribute('aria-valuenow', String(Math.round(shownPercent)));
  }

  function sourcePercentOf(playground) {
    const playgroundStyle = getComputedStyle(playground);
    const curtainWidth = playgroundStyle.getPropertyValue('--svugy-curtain');
    return parseFloat(curtainWidth);
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

  // Render off the page's thread in a worker, so typing never waits on the search; where a worker cannot start, render on the page.
  async function startRenderer() {
    try {
      const workerWasmBytes = decodedWasmBytes();
      return await startWorkerRenderer(workerWasmBytes);
    } catch {
      const pageWasmBytes = decodedWasmBytes();
      return startPageRenderer(pageWasmBytes);
    }
  }

  function discardRenderer() {
    const discardedRendererPromise = sharedRendererPromise;
    sharedRendererPromise = null;
    discardedRendererPromise?.then((renderer) => renderer.close(), () => {});
  }

  function decodedWasmBytes() {
    const binary = atob(encodedWasm);
    return Uint8Array.from(binary, (character) => character.charCodeAt(0));
  }

  // The worker is built from a blob, so it starts from file:// too; its script is the wasm wrapper and the request loop.
  function startWorkerRenderer(wasmBytes) {
    const workerSource = `${wasmRendererOf.toString()}\n(${rendererWorker.toString()})();`;
    const workerBlob = new Blob([workerSource], { type: 'text/javascript' });
    const workerUrl = URL.createObjectURL(workerBlob);
    const worker = new Worker(workerUrl);
    const verdictResolvers = new Map();
    let nextRequestId = 0;
    const renderer = {
      render(request) {
        const id = nextRequestId;
        nextRequestId += 1;
        const verdictPromise = new Promise((resolve, reject) => verdictResolvers.set(id, { resolve, reject }));
        worker.postMessage({ type: 'render', id, ...request });
        return verdictPromise;
      },
      close() {
        worker.terminate();
        URL.revokeObjectURL(workerUrl);
      },
    };
    return new Promise((resolve, reject) => {
      worker.addEventListener('error', (event) => reject(event.error ?? event.message), { once: true });
      worker.addEventListener('message', (event) => {
        const message = event.data;
        if (message.type === 'ready') {
          resolve(renderer);
          return;
        }
        if (message.type === 'failed') {
          renderer.close();
          reject(message.reason);
          return;
        }
        const verdictResolver = verdictResolvers.get(message.id);
        verdictResolvers.delete(message.id);
        if (message.error) {
          verdictResolver.reject(message.error);
          return;
        }
        verdictResolver.resolve(message.verdict);
      });
      worker.postMessage({ type: 'init', wasmBytes }, [wasmBytes.buffer]);
    });
  }

  // The fallback blocks the page while it renders, but yields between the draft and the search so the draft paints first.
  async function startPageRenderer(wasmBytes) {
    const { instance } = await WebAssembly.instantiate(wasmBytes, {});
    const renderOnWasm = wasmRendererOf(instance);
    return {
      async render({ text, languageByte, themeByte, isDraft }) {
        await new Promise((resolve) => setTimeout(resolve, 0));
        return renderOnWasm(text, languageByte, themeByte, isDraft);
      },
      close() {},
    };
  }

  // Wrap one wasm instance of svugy into one function: source text in, verdict out, drawn in the theme named, drafted in the seed arrangement when asked.
  function wasmRendererOf(instance) {
    const { memory, input_buffer, render, render_draft, output_pointer } = instance.exports;
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    return (text, languageByte, themeByte, isDraft) => {
      const encoded = encoder.encode(text);
      const inputStart = input_buffer(encoded.length);
      const inputView = new Uint8Array(memory.buffer, inputStart, encoded.length);
      inputView.set(encoded);
      const renderVerdict = isDraft ? render_draft : render;
      const outputLength = renderVerdict(languageByte, themeByte);
      const outputStart = output_pointer();
      const output = new Uint8Array(memory.buffer, outputStart, outputLength);
      const verdictJson = decoder.decode(output);
      return JSON.parse(verdictJson);
    };
  }

  // Runs inside the worker: renders requests in arrival order, answering null for one a newer request from the same editor has made stale.
  function rendererWorker() {
    let renderOnWasm = null;
    const queue = [];
    let isDrainScheduled = false;

    self.addEventListener('message', async (event) => {
      const message = event.data;
      if (message.type === 'init') {
        try {
          const { instance } = await WebAssembly.instantiate(message.wasmBytes, {});
          renderOnWasm = wasmRendererOf(instance);
          self.postMessage({ type: 'ready' });
        } catch (error) {
          self.postMessage({ type: 'failed', reason: String(error) });
        }
        return;
      }
      queue.push(message);
      scheduleDrain();
    });

    // Draining one request per task lets requests sent during a long search arrive before the next one starts.
    function scheduleDrain() {
      if (isDrainScheduled) return;
      isDrainScheduled = true;
      setTimeout(drainOne, 0);
    }

    function drainOne() {
      isDrainScheduled = false;
      const request = queue.shift();
      if (!request) return;
      const isStale = queue.some((queued) => queued.editorKey === request.editorKey && queued.generation > request.generation);
      try {
        const verdict = isStale ? null : renderOnWasm(request.text, request.languageByte, request.themeByte, request.isDraft);
        self.postMessage({ type: 'verdict', id: request.id, verdict });
      } catch (error) {
        self.postMessage({ type: 'verdict', id: request.id, error: String(error) });
      }
      if (queue.length > 0) scheduleDrain();
    }
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
