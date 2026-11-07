(() => {
  'use strict';

  const COPIED_ICON_MS = 1500;
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
  /** Copies the published skill as a prompt, its relative reference link made absolute so it still resolves once pasted. */
  async function bindSkillCopyButton(button) {
    prependCopyIcons(button);
    const response = await fetch('skill/SKILL.md');
    if (!response.ok) return;
    const skillMarkdown = await response.text();
    const referenceUrl = new URL('skill/reference.md', document.baseURI);
    const skillPrompt = skillMarkdown.replace('](reference.md)', `](${referenceUrl.href})`);
    button.disabled = false;
    button.addEventListener('click', async () => {
      await navigator.clipboard.writeText(skillPrompt);
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

  const installRoot = document.querySelector('[data-tabs]');
  bindInstallTabs(installRoot);
  bindCommandCopyButtons();
  const skillCopyButton = document.querySelector('[data-copy-skill]');
  bindSkillCopyButton(skillCopyButton);
  const source = document.querySelector('.svugy-live textarea');
  const examplePicker = document.querySelector('[data-example-picker]');
  examplePicker.addEventListener('change', () => loadExampleIntoEditor(examplePicker.value, source));
  loadExampleIntoEditor(examplePicker.value, source).then(loadLiveEditorScripts);
})();
