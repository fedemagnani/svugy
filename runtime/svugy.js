(() => {
  'use strict';

  const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';

  const CONTRACT = 'data-svugy-contract';
  const LEVEL = 'data-svugy-level';
  const VIEW = 'data-svugy-view';
  const ELEMENT = 'data-svugy-element';
  const EDGE = 'data-svugy-edge';
  const STUB = 'data-svugy-stub';
  const GOTO = 'data-svugy-goto';
  const DETAILS = 'data-svugy-details';
  const ENTERED = 'data-svugy-entered';

  const LEVEL_SELECTOR = `[${LEVEL}]`;
  const ELEMENT_SELECTOR = `[${ELEMENT}]`;
  const ENTERED_SELECTOR = `[${ENTERED}]`;
  const INTERACTIVE_SELECTOR = `[${ELEMENT}],[${EDGE}],[${STUB}]`;

  const ROOT_LEVEL_PATH = '';
  const TWEEN_DURATION_MS = 450;
  const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';

  const PANEL_PADDING_PX = 8;
  const PANEL_LINE_HEIGHT_PX = 16;
  const PANEL_BASELINE_RAISE_PX = 4;
  const PANEL_POINTER_OFFSET_PX = 14;
  const PANEL_VIEW_MARGIN_PX = 6;
  const PANEL_MAX_LINE_CHARACTERS = 72;
  const ARROW = '\u2192';
  const ELLIPSIS = '\u2026';

  /** Binds the runtime to its enclosing diagram once the document is parsed. */
  function start() {
    const diagram = findEnclosingDiagram();
    if (diagram === null) {
      return;
    }
    const isDocumentParsing = document.readyState === 'loading';
    if (isDocumentParsing) {
      document.addEventListener('DOMContentLoaded', () => bindRuntime(diagram), { once: true });
      return;
    }
    bindRuntime(diagram);
  }

  /** Finds the svugy root that holds this script, or the standalone document's root. */
  function findEnclosingDiagram() {
    const script = document.currentScript ?? null;
    const enclosingDiagram = script === null ? null : script.closest(`[${CONTRACT}]`);
    if (enclosingDiagram !== null) {
      return enclosingDiagram;
    }
    const documentRoot = document.documentElement;
    const isDocumentRootDiagram = documentRoot.hasAttribute(CONTRACT);
    return isDocumentRootDiagram ? documentRoot : null;
  }

  /** Wires navigation and detail panels onto one diagram. */
  function bindRuntime(svg) {
    const rootLevel = Contract.levelAt(svg, ROOT_LEVEL_PATH);
    const rootView = View.parse(svg.getAttribute('viewBox'));
    const isDiagramMalformed = rootLevel === null || rootView === null;
    if (isDiagramMalformed) {
      return;
    }
    const animator = new ViewBoxAnimator(svg);
    const navigator = new LevelNavigator(svg, rootLevel, rootView, animator);
    const panel = new DetailPanel(svg);
    Interactions.bind(svg, navigator, panel);
  }

  /** Answers questions about the rendered document by comparing attribute values, never by building selectors from paths. */
  const Contract = {
    levelAt(svg, path) {
      const levels = Array.from(svg.querySelectorAll(LEVEL_SELECTOR));
      const matchingLevel = levels.find((level) => level.getAttribute(LEVEL) === path);
      return matchingLevel ?? null;
    },

    levelDisplaying(svg, path) {
      const level = Contract.levelAt(svg, path);
      if (level !== null) {
        return level;
      }
      const elements = Array.from(svg.querySelectorAll(ELEMENT_SELECTOR));
      const matchingElement = elements.find((element) => element.getAttribute(ELEMENT) === path);
      return matchingElement === undefined ? null : Contract.enclosingLevelOf(matchingElement);
    },

    enclosingLevelOf(node) {
      const parent = node.parentElement;
      return parent === null ? null : parent.closest(LEVEL_SELECTOR);
    },

    interiorLevelOf(group) {
      const path = group.getAttribute(ELEMENT);
      if (path === null) {
        return null;
      }
      const children = Array.from(group.children);
      const interiorLevel = children.find((child) => child.getAttribute(LEVEL) === path);
      return interiorLevel ?? null;
    },

    outermostInteriorBetween(baseLevel, group) {
      let outermostInterior = null;
      let element = group.closest(ELEMENT_SELECTOR);
      while (element !== null) {
        const isOutsideBaseLevel = !baseLevel.contains(element);
        if (isOutsideBaseLevel) {
          break;
        }
        outermostInterior = Contract.interiorLevelOf(element) ?? outermostInterior;
        element = Contract.enclosingElementOf(element);
      }
      return outermostInterior;
    },

    enclosingElementOf(node) {
      const parent = node.parentElement;
      return parent === null ? null : parent.closest(ELEMENT_SELECTOR);
    },

    isEnterable(group) {
      const hasGoto = group.hasAttribute(GOTO);
      const isStub = group.hasAttribute(STUB);
      const interiorLevel = Contract.interiorLevelOf(group);
      return hasGoto || isStub || interiorLevel !== null;
    },

    titleOf(group) {
      const children = Array.from(group.children);
      const title = children.find((child) => child.localName === 'title');
      return title === undefined ? null : title.textContent;
    },

    detailsOf(group) {
      const children = Array.from(group.children);
      const detailsCarrier = children.find((child) => child.hasAttribute(DETAILS));
      if (detailsCarrier === undefined) {
        return null;
      }
      try {
        const details = JSON.parse(detailsCarrier.textContent);
        const isObject = typeof details === 'object' && details !== null;
        return isObject ? details : null;
      } catch {
        return null;
      }
    },
  };

  /** Parses and interpolates "min-x min-y width height" views. */
  const View = {
    parse(text) {
      if (text === null) {
        return null;
      }
      const numbers = text.trim().split(/[\s,]+/).map(Number);
      const [x, y, width, height] = numbers;
      const isWellFormed = numbers.length === 4 && numbers.every(Number.isFinite) && width > 0 && height > 0;
      return isWellFormed ? { x, y, width, height } : null;
    },

    fromRect(rect) {
      return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    },

    interpolate(start, target, fraction) {
      return {
        x: start.x + (target.x - start.x) * fraction,
        y: start.y + (target.y - start.y) * fraction,
        width: start.width + (target.width - start.width) * fraction,
        height: start.height + (target.height - start.height) * fraction,
      };
    },

    // ease(t) = 4t³ for t < ½, else 1 − (2 − 2t)³ / 2
    easeInOutCubic(fraction) {
      const isFirstHalf = fraction < 0.5;
      return isFirstHalf ? 4 * fraction ** 3 : 1 - (2 - 2 * fraction) ** 3 / 2;
    },
  };

  /** Tweens the root viewBox toward a target view, or jumps under reduced motion. */
  class ViewBoxAnimator {
    constructor(svg) {
      this.svg = svg;
      this.frameRequest = 0;
    }

    tweenTo(targetView) {
      cancelAnimationFrame(this.frameRequest);
      const isMotionReduced = window.matchMedia(REDUCED_MOTION_QUERY).matches;
      if (isMotionReduced) {
        this.show(targetView);
        return;
      }
      const startView = View.fromRect(this.svg.viewBox.baseVal);
      let startTime = null;
      const step = (now) => {
        startTime ??= now;
        const elapsedFraction = Math.min(1, (now - startTime) / TWEEN_DURATION_MS);
        const easedFraction = View.easeInOutCubic(elapsedFraction);
        const view = View.interpolate(startView, targetView, easedFraction);
        this.show(view);
        const isTweenRunning = elapsedFraction < 1;
        if (isTweenRunning) {
          this.frameRequest = requestAnimationFrame(step);
        }
      };
      this.frameRequest = requestAnimationFrame(step);
    }

    show(view) {
      this.svg.setAttribute('viewBox', `${view.x} ${view.y} ${view.width} ${view.height}`);
    }
  }

  /** Keeps exactly one level entered and moves the view to it. */
  class LevelNavigator {
    constructor(svg, rootLevel, rootView, animator) {
      this.svg = svg;
      this.rootLevel = rootLevel;
      this.rootView = rootView;
      this.animator = animator;
      const staleEnteredLevels = svg.querySelectorAll(ENTERED_SELECTOR);
      staleEnteredLevels.forEach((level) => level.removeAttribute(ENTERED));
      rootLevel.setAttribute(ENTERED, '');
      this.enteredLevel = rootLevel;
    }

    levelReachedBy(group) {
      const gotoPath = group.getAttribute(GOTO);
      const hasGoto = gotoPath !== null;
      if (hasGoto) {
        const gotoLevel = Contract.levelDisplaying(this.svg, gotoPath);
        return gotoLevel ?? this.enteredLevel;
      }
      const baseLevel = this.enteredOrEnclosingLevelContaining(group);
      const outermostInterior = Contract.outermostInteriorBetween(baseLevel, group);
      return outermostInterior ?? baseLevel;
    }

    enteredOrEnclosingLevelContaining(group) {
      for (let level = this.enteredLevel; level !== null; level = Contract.enclosingLevelOf(level)) {
        const isGroupInside = level.contains(group);
        if (isGroupInside) {
          return level;
        }
      }
      return this.rootLevel;
    }

    exitOneLevel() {
      const parentLevel = Contract.enclosingLevelOf(this.enteredLevel);
      if (parentLevel === null) {
        return;
      }
      this.enter(parentLevel);
    }

    enter(level) {
      const isEnteredLevel = level === this.enteredLevel;
      if (isEnteredLevel) {
        return;
      }
      this.enteredLevel.removeAttribute(ENTERED);
      level.setAttribute(ENTERED, '');
      this.enteredLevel = level;
      const targetView = this.viewOf(level);
      if (targetView === null) {
        return;
      }
      this.animator.tweenTo(targetView);
    }

    viewOf(level) {
      const isRootLevel = level === this.rootLevel;
      if (isRootLevel) {
        return this.rootView;
      }
      return View.parse(level.getAttribute(VIEW));
    }
  }

  /** Shows a group's details beside a client point, at a constant on-screen size inside the current view. */
  class DetailPanel {
    constructor(svg) {
      this.svg = svg;
      this.panel = createSvgElement('g', { class: 'svugy-panel', 'aria-hidden': 'true' });
      this.shownGroup = null;
      this.width = 0;
      this.height = 0;
    }

    show(group, clientPoint) {
      const isShownGroup = group === this.shownGroup;
      if (isShownGroup) {
        this.placeNear(clientPoint);
        return;
      }
      const details = Contract.detailsOf(group);
      const lines = DetailLines.of(details);
      const isDetailless = lines.length === 0;
      if (isDetailless) {
        this.hide();
        return;
      }
      this.render(lines);
      this.shownGroup = group;
      this.placeNear(clientPoint);
    }

    hide() {
      this.panel.remove();
      this.shownGroup = null;
    }

    render(lines) {
      const plate = createSvgElement('rect', { class: 'svugy-panel-plate', rx: 6 });
      const texts = lines.map((line, index) => {
        const baseline = PANEL_PADDING_PX + (index + 1) * PANEL_LINE_HEIGHT_PX - PANEL_BASELINE_RAISE_PX;
        const text = createSvgElement('text', { class: `svugy-panel-${line.role}`, x: PANEL_PADDING_PX, y: baseline });
        text.textContent = DetailLines.truncate(line.text);
        return text;
      });
      this.panel.replaceChildren(plate, ...texts);
      this.svg.appendChild(this.panel);
      const textWidths = texts.map((text) => text.getComputedTextLength());
      this.width = Math.max(...textWidths) + 2 * PANEL_PADDING_PX;
      this.height = lines.length * PANEL_LINE_HEIGHT_PX + 2 * PANEL_PADDING_PX - PANEL_BASELINE_RAISE_PX;
      plate.setAttribute('width', String(this.width));
      plate.setAttribute('height', String(this.height));
    }

    placeNear(clientPoint) {
      const screenMatrix = this.svg.getScreenCTM();
      if (screenMatrix === null) {
        return;
      }
      const userToScreenMatrix = screenMatrix.inverse();
      const clientDomPoint = new DOMPoint(clientPoint.x, clientPoint.y);
      const pointer = clientDomPoint.matrixTransform(userToScreenMatrix);
      const userUnitsPerPixel = 1 / Math.hypot(screenMatrix.a, screenMatrix.b);
      const offset = PANEL_POINTER_OFFSET_PX * userUnitsPerPixel;
      const margin = PANEL_VIEW_MARGIN_PX * userUnitsPerPixel;
      const view = this.svg.viewBox.baseVal;
      const scaledWidth = this.width * userUnitsPerPixel;
      const scaledHeight = this.height * userUnitsPerPixel;
      const x = panelStartBesidePointer(pointer.x, offset, scaledWidth, view.x, view.width, margin);
      const y = panelStartBesidePointer(pointer.y, offset, scaledHeight, view.y, view.height, margin);
      this.panel.setAttribute('transform', `translate(${x} ${y}) scale(${userUnitsPerPixel})`);
    }
  }

  // start = p + o, or p − o − e when that overflows v + w − m; then max(start, v + m)
  function panelStartBesidePointer(pointer, offset, extent, viewStart, viewExtent, margin) {
    const preferredStart = pointer + offset;
    const viewEnd = viewStart + viewExtent - margin;
    const isOverflowingView = preferredStart + extent > viewEnd;
    const start = isOverflowingView ? pointer - offset - extent : preferredStart;
    return Math.max(start, viewStart + margin);
  }

  /** Turns details JSON into panel lines, one builder per details kind. */
  const DetailLines = {
    of(details) {
      if (details === null) {
        return [];
      }
      const linesForKind = DETAIL_LINES_BY_KIND.get(details.kind);
      if (linesForKind === undefined) {
        return [];
      }
      const lines = linesForKind(details);
      return lines.filter((line) => line !== null);
    },

    ofElement(details) {
      const titleLine = DetailLines.line('title', details.label ?? details.path);
      const captionLine = DetailLines.line('body', details.caption);
      const hasLabel = details.label !== undefined;
      const pathLine = hasLabel ? DetailLines.line('body', details.path) : null;
      const metadata = DetailLines.isPlainObject(details.metadata) ? details.metadata : {};
      const metadataLines = Object.entries(metadata).map(([key, value]) => {
        const valueText = typeof value === 'string' ? value : JSON.stringify(value);
        return DetailLines.line('body', `${key}: ${valueText}`);
      });
      return [titleLine, captionLine, pathLine, ...metadataLines];
    },

    ofEdge(details) {
      const titleLine = DetailLines.line('title', details.label ?? details.edge);
      const hasEndpoints = details.from !== undefined && details.to !== undefined;
      const endpointsLine = hasEndpoints ? DetailLines.line('body', `${details.from} ${ARROW} ${details.to}`) : null;
      return [titleLine, endpointsLine];
    },

    ofStub(details) {
      const titleLine = DetailLines.line('title', `${ARROW} ${details.target}`);
      const edges = Array.isArray(details.edges) ? details.edges : [];
      const plainEdges = edges.filter(DetailLines.isPlainObject);
      const edgeLines = plainEdges.flatMap(DetailLines.ofEdge);
      return [titleLine, ...edgeLines];
    },

    line(role, value) {
      const isAbsent = value === undefined || value === null;
      return isAbsent ? null : { role, text: String(value) };
    },

    isPlainObject(value) {
      return typeof value === 'object' && value !== null && !Array.isArray(value);
    },

    truncate(text) {
      const isTooLong = text.length > PANEL_MAX_LINE_CHARACTERS;
      return isTooLong ? `${text.slice(0, PANEL_MAX_LINE_CHARACTERS - 1)}${ELLIPSIS}` : text;
    },
  };

  const DETAIL_LINES_BY_KIND = new Map([
    ['element', DetailLines.ofElement],
    ['edge', DetailLines.ofEdge],
    ['stub', DetailLines.ofStub],
  ]);

  /** Translates pointer, focus and key events into navigation and detail panels. */
  const Interactions = {
    bind(svg, navigator, panel) {
      Interactions.makeGroupsFocusable(svg);
      svg.addEventListener('click', (event) => {
        const group = Interactions.groupAt(event.target);
        const clientPoint = { x: event.clientX, y: event.clientY };
        if (group === null) {
          Interactions.exitOneLevel(navigator, panel);
          return;
        }
        Interactions.activate(group, clientPoint, navigator, panel);
      });
      svg.addEventListener('keydown', (event) => {
        const isEscape = event.key === 'Escape';
        if (isEscape) {
          Interactions.exitOneLevel(navigator, panel);
          return;
        }
        const isActivationKey = event.key === 'Enter' || event.key === ' ';
        const isFocusedGroup = event.target instanceof Element && event.target.matches(INTERACTIVE_SELECTOR);
        const isGroupActivated = isActivationKey && isFocusedGroup;
        if (isGroupActivated) {
          event.preventDefault();
          const groupCenter = Interactions.centerOf(event.target);
          Interactions.activate(event.target, groupCenter, navigator, panel);
        }
      });
      svg.addEventListener('pointermove', (event) => {
        const group = Interactions.groupAt(event.target);
        if (group === null) {
          panel.hide();
          return;
        }
        const clientPoint = { x: event.clientX, y: event.clientY };
        panel.show(group, clientPoint);
      });
      svg.addEventListener('pointerleave', () => panel.hide());
      svg.addEventListener('focusin', (event) => {
        const group = Interactions.groupAt(event.target);
        if (group === null) {
          return;
        }
        const groupCenter = Interactions.centerOf(group);
        panel.show(group, groupCenter);
      });
      svg.addEventListener('focusout', () => panel.hide());
    },

    makeGroupsFocusable(svg) {
      svg.setAttribute('tabindex', '-1');
      const groups = svg.querySelectorAll(INTERACTIVE_SELECTOR);
      groups.forEach((group) => {
        group.setAttribute('tabindex', '0');
        const title = Contract.titleOf(group);
        if (title !== null) {
          group.setAttribute('aria-label', title);
        }
        const isEnterable = Contract.isEnterable(group);
        if (isEnterable) {
          group.setAttribute('role', 'button');
        }
      });
    },

    activate(group, clientPoint, navigator, panel) {
      const reachedLevel = navigator.levelReachedBy(group);
      const isEnteredLevel = reachedLevel === navigator.enteredLevel;
      if (isEnteredLevel) {
        panel.show(group, clientPoint);
        return;
      }
      panel.hide();
      navigator.enter(reachedLevel);
    },

    exitOneLevel(navigator, panel) {
      panel.hide();
      navigator.exitOneLevel();
    },

    groupAt(target) {
      const isElementTarget = target instanceof Element;
      return isElementTarget ? target.closest(INTERACTIVE_SELECTOR) : null;
    },

    centerOf(group) {
      const bounds = group.getBoundingClientRect();
      return { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 };
    },
  };

  function createSvgElement(name, attributes) {
    const element = document.createElementNS(SVG_NAMESPACE, name);
    Object.entries(attributes).forEach(([attribute, value]) => element.setAttribute(attribute, String(value)));
    return element;
  }
  start();
})();
