(() => {
  'use strict';

  const COPIED_ICON_MS = 1500;
  const TYPE_TITLE_CHARACTER_MS = 35;
  const TYPE_WORD_CHARACTER_MS = 90;
  const TITLE_START_DELAY_MS = 150;
  const ERASE_CHARACTER_MS = 50;
  const WORD_REST_MS = 1600;
  const EMPTY_REST_MS = 350;
  // loaded after the first example shows: the renderer is 5 MB, and playground.js mounts as soon as it runs
  const LIVE_EDITOR_SCRIPTS = ['docs/playground/svugy-wasm.js', 'docs/playground/playground.js'];

  /** Shows the install panel of the clicked tab. */
  function bindInstallTabs(root) {
    const tabs = [...root.querySelectorAll('[role="tab"]')];
    tabs.forEach((tab) => {
      tab.addEventListener('click', () => {
        tabs.forEach((other) => {
          const isClicked = other === tab;
          const panel = document.getElementById(other.getAttribute('aria-controls'));
          other.setAttribute('aria-selected', String(isClicked));
          other.tabIndex = isClicked ? 0 : -1;
          panel.hidden = !isClicked;
        });
      });
    });
  }

  /** Copies the command beside each command's copy button. */
  function bindCommandCopyButtons() {
    document.querySelectorAll('[data-copy-command]').forEach((button) => {
      prependCopyIcons(button);
      button.addEventListener('click', async () => {
        const command = button.parentElement.querySelector('code').textContent;
        await navigator.clipboard.writeText(command);
        showCopied(button);
      });
    });
  }

  // Fetched ahead of the click: Safari only lets a click write the clipboard if nothing is awaited first.
  /** Copies the published installation prompt, which tells an agent how to install svugy and its skill. */
  async function bindInstallPromptCopyButton(button) {
    prependCopyIcons(button);
    const response = await fetch('install.md');
    if (!response.ok) return;
    const installPrompt = await response.text();
    button.disabled = false;
    button.addEventListener('click', async () => {
      await navigator.clipboard.writeText(installPrompt);
      showCopied(button);
    });
  }

  function prependCopyIcons(button) {
    const copyIcons = document.getElementById('copy-icons');
    const iconNodes = copyIcons.content.cloneNode(true);
    button.prepend(iconNodes);
  }

  // the check icon shows instead of the copy icon for a moment
  function showCopied(button) {
    button.dataset.copied = '';
    setTimeout(() => delete button.dataset.copied, COPIED_ICON_MS);
  }

  /** Types the title with its first word, then erases its last word and types the next one, forever; under reduced motion the title stays as written. */
  async function typeTitleForever(typedTitle) {
    const isMotionReduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (isMotionReduced) return;
    const prefix = `${typedTitle.dataset.typedPrefix} `;
    const words = typedTitle.dataset.typedWords.split(' ');
    const firstTitle = prefix + words[0];
    typedTitle.textContent = '';
    await sleep(TITLE_START_DELAY_MS);
    await typeUpTo(typedTitle, firstTitle, TYPE_TITLE_CHARACTER_MS);
    for (let wordIndex = 0; ; wordIndex = (wordIndex + 1) % words.length) {
      await sleep(WORD_REST_MS);
      await eraseDownTo(typedTitle, prefix);
      await sleep(EMPTY_REST_MS);
      const nextWordIndex = (wordIndex + 1) % words.length;
      const nextTitle = prefix + words[nextWordIndex];
      await typeUpTo(typedTitle, nextTitle, TYPE_WORD_CHARACTER_MS);
    }
  }

  // the shown text is always a prefix of `fullText`
  async function typeUpTo(typedTitle, fullText, characterMs) {
    typedTitle.dataset.typing = '';
    const typedLength = typedTitle.textContent.length;
    for (let length = typedLength + 1; length <= fullText.length; length++) {
      typedTitle.textContent = fullText.slice(0, length);
      await sleep(characterMs);
    }
    delete typedTitle.dataset.typing;
  }

  // `keptText` is always a prefix of the shown text
  async function eraseDownTo(typedTitle, keptText) {
    typedTitle.dataset.typing = '';
    const shownText = typedTitle.textContent;
    for (let length = shownText.length - 1; length >= keptText.length; length--) {
      typedTitle.textContent = shownText.slice(0, length);
      await sleep(ERASE_CHARACTER_MS);
    }
    delete typedTitle.dataset.typing;
  }

  function sleep(milliseconds) {
    return new Promise((resolve) => setTimeout(resolve, milliseconds));
  }

  /** Puts an example's Markdown in the editor; the live editor re-renders it on the input event. */
  async function loadExampleIntoEditor(exampleName, source) {
    const response = await fetch(`models/${exampleName}.md`);
    const markdown = await response.text();
    source.value = markdown;
    source.scrollTop = 0;
    const inputEvent = new Event('input');
    source.dispatchEvent(inputEvent);
  }

  function loadLiveEditorScripts() {
    for (const scriptPath of LIVE_EDITOR_SCRIPTS) {
      const script = document.createElement('script');
      script.src = scriptPath;
      script.async = false;
      document.body.append(script);
    }
  }

  const typedTitle = document.querySelector('.typed-title');
  typeTitleForever(typedTitle);
  const installRoot = document.querySelector('[data-tabs]');
  bindInstallTabs(installRoot);
  bindCommandCopyButtons();
  const installPromptCopyButton = document.querySelector('[data-copy-install-prompt]');
  bindInstallPromptCopyButton(installPromptCopyButton);
  const source = document.querySelector('.svugy-live textarea');
  const examplePicker = document.querySelector('[data-example-picker]');
  examplePicker.addEventListener('change', () => loadExampleIntoEditor(examplePicker.value, source));
  loadExampleIntoEditor(examplePicker.value, source).then(loadLiveEditorScripts);
})();
