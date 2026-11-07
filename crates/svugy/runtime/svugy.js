(() => {
  'use strict';

  const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';

  const CONTRACT = 'data-svugy-contract';
  const LAYER = 'data-svugy-layer';
  const SHOWN = 'data-svugy-shown';
  const LEVEL = 'data-svugy-level';
  const VIEW = 'data-svugy-view';
  const ELEMENT = 'data-svugy-element';
  const CLOSED = 'data-svugy-closed';
  const EDGE = 'data-svugy-edge';
  const FROM = 'data-svugy-from';
  const TO = 'data-svugy-to';
  const STUB = 'data-svugy-stub';
  const GOTO = 'data-svugy-goto';
  const OVERLAY = 'data-svugy-overlay';
  const TRAVELER = 'data-svugy-traveler';
  const DETAILS = 'data-svugy-details';
  const ENTERED = 'data-svugy-entered';
  const DIMMED = 'data-svugy-dimmed';
  const CONTRACT_ATTRIBUTE_PREFIX = 'data-svugy-';

  const LAYER_SELECTOR = `[${LAYER}]`;
  const LEVEL_SELECTOR = `[${LEVEL}]`;
  const ELEMENT_SELECTOR = `[${ELEMENT}]`;
  const EDGE_SELECTOR = `[${EDGE}]`;
  const SHOWN_SELECTOR = `[${SHOWN}]`;
  const ENTERED_SELECTOR = `[${ENTERED}]`;
  const DIMMED_SELECTOR = `[${DIMMED}]`;
  const INTERACTIVE_SELECTOR = `[${ELEMENT}],[${EDGE}],[${STUB}]`;
  const ELEMENT_OR_LEVEL_SELECTOR = `[${ELEMENT}],[${LEVEL}]`;
  const NON_PAINTING_SELECTOR = `[${OVERLAY}],[${TRAVELER}],title,desc`;
  const MINIATURE_STRIPPED_ATTRIBUTES = new Set(['id', 'tabindex', 'role', 'aria-label', 'style']);

  const ROOT_LEVEL_PATH = '';
  const TWEEN_DURATION_MS = 450;
  const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';
  const FADE_IN_KEYFRAMES = [{ opacity: 0 }, { opacity: 1 }];
  const FADE_OUT_KEYFRAMES = [{ opacity: 1 }, { opacity: 0 }];

  const PANEL_PADDING_PX = 8;
  const PANEL_LINE_HEIGHT_PX = 16;
  const PANEL_BASELINE_RAISE_PX = 4;
  const PANEL_POINTER_OFFSET_PX = 14;
  const PANEL_VIEW_MARGIN_PX = 6;
  const PANEL_MAX_LINE_CHARACTERS = 72;

  const ELEVATOR_PADDING_PX = 6;
  const ELEVATOR_VIEW_MARGIN_PX = 10;
  const ELEVATOR_ROW_HEIGHT_PX = 20;
  const ELEVATOR_ROW_BASELINE_PX = 14;
  const ELEVATOR_FLOOR_INDENT_PX = 10;
  const ELEVATOR_LABEL_INSET_PX = 8;
  const ELEVATOR_MAP_MAX_WIDTH_PX = 176;
  const ELEVATOR_MAP_MAX_HEIGHT_PX = 120;
  const ROOT_FLOOR_FALLBACK_LABEL = 'root';

  const ARROW = '\u2192';
  const ELLIPSIS = '\u2026';
  const TOGGLE_GLYPH = '\u21c4';

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

  /** Wires navigation, detail panels, dimming and the elevator onto one diagram. */
  function bindRuntime(svg) {
    const layers = Layers.of(svg);
    const initialLayer = Layers.initiallyShown(layers);
    const rootLevel = Contract.levelAt(initialLayer, ROOT_LEVEL_PATH);
    if (rootLevel === null) {
      return;
    }
    const animator = new ViewBoxAnimator(svg);
    const fader = new LayerFader();
    const navigator = new LayerNavigator(svg, layers, initialLayer, rootLevel, animator, fader);
    const panel = new DetailPanel(svg);
    const dimmer = new UnrelatedGroupDimmer(svg);
    const elevator = new Elevator(svg, (level) => navigator.focus(level));
    navigator.addNavigationListener((focusedLevel) => {
      panel.hide();
      dimmer.clear();
      elevator.showFor(focusedLevel);
    });
    Interactions.bind(svg, navigator, panel, dimmer);
  }

  /** Lists the layers a document draws, in increasing depth; a document without layer groups is its own single layer. */
  const Layers = {
    of(svg) {
      const layerGroups = Array.from(svg.querySelectorAll(LAYER_SELECTOR));
      const hasLayerGroups = layerGroups.length > 0;
      return hasLayerGroups ? layerGroups : [svg];
    },

    initiallyShown(layers) {
      const renderedShownLayer = layers.find((layer) => layer.hasAttribute(SHOWN));
      return renderedShownLayer ?? layers[0];
    },
  };

  /** Answers questions about the rendered document by comparing attribute values, never by building selectors from paths. */
  const Contract = {
    levelAt(scope, path) {
      const levels = Array.from(scope.querySelectorAll(LEVEL_SELECTOR));
      const matchingLevel = levels.find((level) => level.getAttribute(LEVEL) === path);
      return matchingLevel ?? null;
    },

    elementAt(scope, path) {
      const elements = Array.from(scope.querySelectorAll(ELEMENT_SELECTOR));
      const matchingElement = elements.find((element) => element.getAttribute(ELEMENT) === path);
      return matchingElement ?? null;
    },

    levelDisplaying(scope, path) {
      const level = Contract.levelAt(scope, path);
      if (level !== null) {
        return level;
      }
      const element = Contract.elementAt(scope, path);
      return element === null ? null : Contract.enclosingLevelOf(element);
    },

    counterpartIn(scope, node) {
      const levelPath = node.getAttribute(LEVEL);
      if (levelPath !== null) {
        return Contract.levelAt(scope, levelPath);
      }
      const elementPath = node.getAttribute(ELEMENT);
      if (elementPath !== null) {
        return Contract.elementAt(scope, elementPath);
      }
      return null;
    },

    enclosingLevelOf(node) {
      const parent = node.parentElement;
      return parent === null ? null : parent.closest(LEVEL_SELECTOR);
    },

    depthOf(level) {
      const levelsFromRoot = Contract.levelsFromRootTo(level);
      return levelsFromRoot.length - 1;
    },

    levelsFromRootTo(level) {
      const levels = [];
      for (let ancestor = level; ancestor !== null; ancestor = Contract.enclosingLevelOf(ancestor)) {
        levels.unshift(ancestor);
      }
      return levels;
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

    ancestorElementsOf(node) {
      const ancestors = [];
      for (let element = Contract.enclosingElementOf(node); element !== null; element = Contract.enclosingElementOf(element)) {
        ancestors.push(element);
      }
      return ancestors;
    },

    endpointsOf(edge) {
      const from = edge.getAttribute(FROM);
      const to = edge.getAttribute(TO);
      return [from, to];
    },

    isBadge(edge) {
      const parent = edge.parentElement;
      const enclosingGroup = parent === null ? null : parent.closest(ELEMENT_OR_LEVEL_SELECTOR);
      return enclosingGroup !== null && enclosingGroup.hasAttribute(ELEMENT);
    },

    isEnterable(group) {
      const hasGoto = group.hasAttribute(GOTO);
      const isStub = group.hasAttribute(STUB);
      const isClosedContainer = group.hasAttribute(CLOSED);
      const interiorLevel = Contract.interiorLevelOf(group);
      const isOpenContainer = interiorLevel !== null;
      return hasGoto || isStub || isClosedContainer || isOpenContainer;
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

    format(view) {
      return `${view.x} ${view.y} ${view.width} ${view.height}`;
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

    tweenTo(targetView, startView = this.currentView()) {
      cancelAnimationFrame(this.frameRequest);
      const isMotionReduced = window.matchMedia(REDUCED_MOTION_QUERY).matches;
      if (isMotionReduced) {
        this.show(targetView);
        return;
      }
      this.show(startView);
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

    currentView() {
      const viewBox = this.svg.viewBox.baseVal;
      return View.fromRect(viewBox);
    }

    show(view) {
      const viewBoxText = View.format(view);
      this.svg.setAttribute('viewBox', viewBoxText);
    }
  }

  /** Crossfades the shown layer into another, or swaps them at once under reduced motion. */
  class LayerFader {
    constructor() {
      this.runningFade = null;
    }

    crossfade(fadingLayer, appearingLayer) {
      this.finishRunningFade();
      appearingLayer.setAttribute(SHOWN, '');
      const isMotionReduced = window.matchMedia(REDUCED_MOTION_QUERY).matches;
      if (isMotionReduced) {
        fadingLayer.removeAttribute(SHOWN);
        return;
      }
      fadingLayer.style.pointerEvents = 'none';
      const timing = { duration: TWEEN_DURATION_MS, easing: 'ease-in-out' };
      const fadeOut = fadingLayer.animate(FADE_OUT_KEYFRAMES, timing);
      const fadeIn = appearingLayer.animate(FADE_IN_KEYFRAMES, timing);
      fadeOut.onfinish = () => this.finishRunningFade();
      this.runningFade = { fadingLayer, fadeOut, fadeIn };
    }

    finishRunningFade() {
      const fade = this.runningFade;
      if (fade === null) {
        return;
      }
      this.runningFade = null;
      fade.fadeOut.cancel();
      fade.fadeIn.cancel();
      fade.fadingLayer.removeAttribute(SHOWN);
      fade.fadingLayer.style.removeProperty('pointer-events');
    }
  }

  /** Keeps one layer shown and one of its levels focused, and moves the view to them. */
  class LayerNavigator {
    constructor(svg, layers, initialLayer, rootLevel, animator, fader) {
      this.svg = svg;
      this.layers = layers;
      this.animator = animator;
      this.fader = fader;
      const staleMarks = svg.querySelectorAll(`${SHOWN_SELECTOR},${ENTERED_SELECTOR}`);
      staleMarks.forEach((node) => {
        node.removeAttribute(SHOWN);
        node.removeAttribute(ENTERED);
      });
      initialLayer.setAttribute(SHOWN, '');
      rootLevel.setAttribute(ENTERED, '');
      this.shownLayer = initialLayer;
      this.focusedLevel = rootLevel;
      this.navigationListeners = [];
    }

    /** Calls `listener(focusedLevel)` after every change of the shown layer or the focused level. */
    addNavigationListener(listener) {
      this.navigationListeners.push(listener);
    }

    /** Follows a click on `group` and tells whether the view moved; when it did not, the group's details are due. */
    followClickOn(group) {
      const gotoPath = group.getAttribute(GOTO);
      const hasGoto = gotoPath !== null;
      if (hasGoto) {
        return this.goTo(gotoPath, group);
      }
      const isClosedContainer = group.hasAttribute(CLOSED);
      if (isClosedContainer) {
        return this.revealOneMoreDepth(group);
      }
      const reachedLevel = this.levelReachedThrough(group);
      return this.focus(reachedLevel);
    }

    goTo(path, group) {
      const openingLayer = this.firstLayerFromShownOpening(path);
      if (openingLayer === null) {
        const displayingLevel = Contract.levelDisplaying(this.shownLayer, path);
        if (displayingLevel === null) {
          return false;
        }
        return this.focus(displayingLevel);
      }
      const isShownLayer = openingLayer === this.shownLayer;
      if (isShownLayer) {
        const openLevel = Contract.levelAt(openingLayer, path);
        return this.focus(openLevel);
      }
      return this.showLayer(openingLayer, path, group);
    }

    firstLayerFromShownOpening(path) {
      const shownLayerIndex = this.layers.indexOf(this.shownLayer);
      const layersFromShown = this.layers.slice(shownLayerIndex);
      const openingLayer = layersFromShown.find((layer) => Contract.levelAt(layer, path) !== null);
      return openingLayer ?? null;
    }

    revealOneMoreDepth(closedContainer) {
      const shownLayerIndex = this.layers.indexOf(this.shownLayer);
      const deeperLayer = this.layers[shownLayerIndex + 1];
      if (deeperLayer === undefined) {
        return false;
      }
      const focusedPath = this.focusedLevel.getAttribute(LEVEL);
      return this.showLayer(deeperLayer, focusedPath, closedContainer);
    }

    levelReachedThrough(group) {
      const baseLevel = this.focusedOrEnclosingLevelContaining(group);
      const outermostInterior = Contract.outermostInteriorBetween(baseLevel, group);
      return outermostInterior ?? baseLevel;
    }

    focusedOrEnclosingLevelContaining(group) {
      for (let level = this.focusedLevel; level !== null; level = Contract.enclosingLevelOf(level)) {
        const isGroupInside = level.contains(group);
        if (isGroupInside) {
          return level;
        }
      }
      return Contract.levelAt(this.shownLayer, ROOT_LEVEL_PATH);
    }

    /** Undoes one step: closes the deepest open containers while the shown layer is deeper than the focused level, else focuses its parent. */
    exitOneStep() {
      const shownLayerIndex = this.layers.indexOf(this.shownLayer);
      const focusedDepth = Contract.depthOf(this.focusedLevel);
      const isLayerDeeperThanFocus = shownLayerIndex > focusedDepth;
      if (isLayerDeeperThanFocus) {
        const shallowerLayer = this.layers[shownLayerIndex - 1];
        const focusedPath = this.focusedLevel.getAttribute(LEVEL);
        this.showLayer(shallowerLayer, focusedPath, this.focusedLevel);
        return;
      }
      const parentLevel = Contract.enclosingLevelOf(this.focusedLevel);
      if (parentLevel === null) {
        return;
      }
      this.focus(parentLevel);
    }

    focus(level) {
      const isFocusedLevel = level === this.focusedLevel;
      if (isFocusedLevel) {
        return false;
      }
      this.moveEnteredMarkTo(level);
      this.announceNavigation();
      const targetView = this.viewOf(level);
      if (targetView !== null) {
        this.animator.tweenTo(targetView);
      }
      return true;
    }

    /** Shows `layer` focused on its level at `focusedPath`, keeping `anchor`'s counterpart where `anchor` was on screen as the view starts moving. */
    showLayer(layer, focusedPath, anchor) {
      const targetLevel = Contract.levelAt(layer, focusedPath);
      if (targetLevel === null) {
        return false;
      }
      const startView = this.anchoredStartView(layer, anchor);
      const previousLayer = this.shownLayer;
      this.moveEnteredMarkTo(targetLevel);
      this.shownLayer = layer;
      this.fader.crossfade(previousLayer, layer);
      this.announceNavigation();
      this.carryKeyboardFocus(previousLayer, layer);
      const targetView = this.viewOf(targetLevel);
      if (targetView !== null) {
        this.animator.tweenTo(targetView, startView);
      }
      return true;
    }

    anchoredStartView(layer, anchor) {
      const currentView = this.animator.currentView();
      const anchorCounterpart = Contract.counterpartIn(layer, anchor);
      const hasAnchorCounterpart = anchorCounterpart !== null;
      const focusedCounterpart = Contract.counterpartIn(layer, this.focusedLevel);
      const oldAnchor = hasAnchorCounterpart ? anchor : this.focusedLevel;
      const newAnchor = hasAnchorCounterpart ? anchorCounterpart : focusedCounterpart;
      if (newAnchor === null) {
        return currentView;
      }
      const oldCenter = this.userCenterOf(oldAnchor);
      const newCenter = this.userCenterOf(newAnchor);
      const isCenterUnmeasurable = oldCenter === null || newCenter === null;
      if (isCenterUnmeasurable) {
        return currentView;
      }
      // start = view + (new center − old center), at the view's size, so the new center lands on the old one's screen point
      return {
        x: currentView.x + newCenter.x - oldCenter.x,
        y: currentView.y + newCenter.y - oldCenter.y,
        width: currentView.width,
        height: currentView.height,
      };
    }

    userCenterOf(node) {
      const screenMatrix = this.svg.getScreenCTM();
      if (screenMatrix === null) {
        return null;
      }
      const screenToUserMatrix = screenMatrix.inverse();
      const bounds = node.getBoundingClientRect();
      const clientCenterX = bounds.left + bounds.width / 2;
      const clientCenterY = bounds.top + bounds.height / 2;
      const clientCenter = new DOMPoint(clientCenterX, clientCenterY);
      return clientCenter.matrixTransform(screenToUserMatrix);
    }

    moveEnteredMarkTo(level) {
      this.focusedLevel.removeAttribute(ENTERED);
      level.setAttribute(ENTERED, '');
      this.focusedLevel = level;
    }

    carryKeyboardFocus(previousLayer, layer) {
      const focusedNode = document.activeElement;
      const isFocusInPreviousLayer = previousLayer.contains(focusedNode);
      const isFocusOutsidePreviousLayer = !isFocusInPreviousLayer;
      if (isFocusOutsidePreviousLayer) {
        return;
      }
      const focusedCounterpart = Contract.counterpartIn(layer, focusedNode);
      const focusTarget = focusedCounterpart ?? this.svg;
      focusTarget.focus({ preventScroll: true });
    }

    announceNavigation() {
      this.navigationListeners.forEach((listener) => listener(this.focusedLevel));
    }

    viewOf(level) {
      const viewText = level.getAttribute(VIEW);
      return View.parse(viewText);
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
      const viewBoxObserver = new MutationObserver(() => this.followShownGroup());
      viewBoxObserver.observe(svg, { attributes: true, attributeFilter: ['viewBox'] });
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

    followShownGroup() {
      const isHidden = this.shownGroup === null;
      if (isHidden) {
        return;
      }
      const groupCenter = clientCenterOf(this.shownGroup);
      this.placeNear(groupCenter);
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
      const screenToUserMatrix = screenMatrix.inverse();
      const clientDomPoint = new DOMPoint(clientPoint.x, clientPoint.y);
      const pointer = clientDomPoint.matrixTransform(screenToUserMatrix);
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
      const endpointsText = DetailLines.endpointsTextOf(details);
      const endpointsLine = DetailLines.line('body', endpointsText);
      return [titleLine, endpointsLine];
    },

    ofLifted(details) {
      const endpointsText = DetailLines.endpointsTextOf(details);
      const titleLine = DetailLines.line('title', details.label ?? endpointsText);
      const sourceEdges = DetailLines.plainObjectsIn(details.edges);
      const sourceEdgeLines = sourceEdges.map(DetailLines.sourceEdgeLine);
      return [titleLine, ...sourceEdgeLines];
    },

    ofStub(details) {
      const titleLine = DetailLines.line('title', `${ARROW} ${details.target}`);
      const connections = DetailLines.plainObjectsIn(details.edges);
      const connectionLines = connections.flatMap((connection) => DetailLines.of(connection));
      return [titleLine, ...connectionLines];
    },

    sourceEdgeLine(edge) {
      const endpointsText = DetailLines.endpointsTextOf(edge) ?? edge.edge;
      const hasLabel = edge.label !== undefined;
      const hasEndpointsText = endpointsText !== undefined;
      const isLabelBesideEndpoints = hasLabel && hasEndpointsText;
      const text = isLabelBesideEndpoints ? `${edge.label}: ${endpointsText}` : edge.label ?? endpointsText;
      return DetailLines.line('body', text);
    },

    endpointsTextOf(details) {
      const hasEndpoints = details.from !== undefined && details.to !== undefined;
      return hasEndpoints ? `${details.from} ${ARROW} ${details.to}` : undefined;
    },

    line(role, value) {
      const isAbsent = value === undefined || value === null;
      return isAbsent ? null : { role, text: String(value) };
    },

    plainObjectsIn(value) {
      const isArray = Array.isArray(value);
      return isArray ? value.filter(DetailLines.isPlainObject) : [];
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
    ['lifted', DetailLines.ofLifted],
    ['stub', DetailLines.ofStub],
  ]);

  /** Dims the element and edge groups of the shown layer unrelated to one hovered or focused group; stubs and badges are never dimmed. */
  class UnrelatedGroupDimmer {
    constructor(svg) {
      const staleMarks = svg.querySelectorAll(DIMMED_SELECTOR);
      staleMarks.forEach((node) => node.removeAttribute(DIMMED));
      this.anchorGroup = null;
      this.dimmedGroups = [];
    }

    dimUnrelatedTo(group, shownLayer) {
      const isAnchorGroup = group === this.anchorGroup;
      if (isAnchorGroup) {
        return;
      }
      this.clear();
      const isGroupOutsideShownLayer = !shownLayer.contains(group);
      if (isGroupOutsideShownLayer) {
        return;
      }
      const isStub = group.hasAttribute(STUB);
      if (isStub) {
        return;
      }
      const relatives = Relatives.of(group, shownLayer);
      const layerElements = Array.from(shownLayer.querySelectorAll(ELEMENT_SELECTOR));
      const layerEdges = Array.from(shownLayer.querySelectorAll(EDGE_SELECTOR));
      const dimmableEdges = layerEdges.filter((edge) => !Contract.isBadge(edge));
      const dimmableGroups = [...layerElements, ...dimmableEdges];
      this.dimmedGroups = dimmableGroups.filter((candidate) => !relatives.has(candidate));
      this.dimmedGroups.forEach((dimmedGroup) => dimmedGroup.setAttribute(DIMMED, ''));
      this.anchorGroup = group;
    }

    clear() {
      this.dimmedGroups.forEach((dimmedGroup) => dimmedGroup.removeAttribute(DIMMED));
      this.dimmedGroups = [];
      this.anchorGroup = null;
    }
  }

  /** Collects the groups of a layer related to one element or edge group: its edges, their endpoints, and the ancestors of all of these. */
  const Relatives = {
    of(group, layer) {
      const isEdge = group.hasAttribute(EDGE);
      const core = isEdge ? Relatives.coreOfEdge(group) : Relatives.coreOfElement(group);
      const layerEdges = Array.from(layer.querySelectorAll(EDGE_SELECTOR));
      const coreEdges = layerEdges.filter(core.isCoreEdge);
      const endpointPaths = new Set(coreEdges.flatMap((edge) => Contract.endpointsOf(edge)));
      const layerElements = Array.from(layer.querySelectorAll(ELEMENT_SELECTOR));
      const endpointElements = layerElements.filter((element) => endpointPaths.has(element.getAttribute(ELEMENT)));
      const litElements = [...core.elements, ...endpointElements];
      const litAncestors = litElements.flatMap((element) => Contract.ancestorElementsOf(element));
      return new Set([...litElements, ...litAncestors, ...coreEdges]);
    },

    /** The element, its descendants, and every edge with an endpoint among them. */
    coreOfElement(element) {
      const descendants = Array.from(element.querySelectorAll(ELEMENT_SELECTOR));
      const subtree = [element, ...descendants];
      const subtreePaths = new Set(subtree.map((member) => member.getAttribute(ELEMENT)));
      const isCoreEdge = (edge) => {
        const [from, to] = Contract.endpointsOf(edge);
        const isFromInSubtree = subtreePaths.has(from);
        const isToInSubtree = subtreePaths.has(to);
        return isFromInSubtree || isToInSubtree;
      };
      return { elements: subtree, isCoreEdge };
    },

    /** Every group drawing the same edge, since a labelled edge has a second group for its label. */
    coreOfEdge(edge) {
      const edgeName = edge.getAttribute(EDGE);
      const isCoreEdge = (candidate) => candidate.getAttribute(EDGE) === edgeName;
      return { elements: [], isCoreEdge };
    },
  };

  /** Shows where the focused level sits, as a chain of floors or a mini-map, pinned top-right at a constant on-screen size; hidden at the root. */
  class Elevator {
    constructor(svg, focusLevel) {
      this.svg = svg;
      const diagramTitle = Contract.titleOf(svg);
      const rootLabel = diagramTitle ?? ROOT_FLOOR_FALLBACK_LABEL;
      this.modes = [new FloorChain(rootLabel, focusLevel), new MiniMap(focusLevel)];
      this.modeIndex = 0;
      this.elevator = createSvgElement('g', { class: 'svugy-elevator', role: 'group', 'aria-label': 'elevator' });
      this.elevator.addEventListener('click', (event) => event.stopPropagation());
      this.focusedLevel = null;
      this.width = 0;
      const viewBoxObserver = new MutationObserver(() => this.place());
      viewBoxObserver.observe(svg, { attributes: true, attributeFilter: ['viewBox'] });
      const resizeObserver = new ResizeObserver(() => this.place());
      resizeObserver.observe(svg);
    }

    showFor(focusedLevel) {
      this.focusedLevel = focusedLevel;
      const parentLevel = Contract.enclosingLevelOf(focusedLevel);
      const isFocusedLevelRoot = parentLevel === null;
      if (isFocusedLevelRoot) {
        this.hide();
        return;
      }
      this.render();
    }

    hide() {
      const wasFocusInside = this.elevator.contains(document.activeElement);
      this.elevator.remove();
      if (wasFocusInside) {
        this.svg.focus({ preventScroll: true });
      }
    }

    render() {
      const wasFocusInside = this.elevator.contains(document.activeElement);
      const mode = this.modes[this.modeIndex];
      const nextModeIndex = (this.modeIndex + 1) % this.modes.length;
      const nextMode = this.modes[nextModeIndex];
      const plate = createSvgElement('rect', { class: 'svugy-elevator-plate', rx: 6 });
      const toggle = ElevatorRow.create('svugy-elevator-toggle', `${TOGGLE_GLYPH} ${nextMode.name}`, 0);
      toggle.setAttribute('transform', `translate(${ELEVATOR_PADDING_PX} ${ELEVATOR_PADDING_PX})`);
      bindButton(toggle, `show the ${nextMode.name}`, () => this.switchToMode(nextModeIndex));
      const contentTop = 2 * ELEVATOR_PADDING_PX + ELEVATOR_ROW_HEIGHT_PX;
      const content = createSvgElement('g', { transform: `translate(${ELEVATOR_PADDING_PX} ${contentTop})` });
      this.elevator.replaceChildren(plate, toggle, content);
      this.svg.appendChild(this.elevator);
      const toggleWidth = ElevatorRow.naturalWidthOf(toggle);
      ElevatorRow.setWidth(toggle, toggleWidth);
      const contentSize = mode.drawInto(content, this.focusedLevel);
      const innerWidth = Math.max(toggleWidth, contentSize.width);
      this.width = innerWidth + 2 * ELEVATOR_PADDING_PX;
      const height = contentTop + contentSize.height + ELEVATOR_PADDING_PX;
      plate.setAttribute('width', String(this.width));
      plate.setAttribute('height', String(height));
      this.place();
      if (wasFocusInside) {
        toggle.focus({ preventScroll: true });
      }
    }

    switchToMode(modeIndex) {
      this.modeIndex = modeIndex;
      this.render();
    }

    place() {
      const isDetached = !this.elevator.isConnected;
      if (isDetached) {
        return;
      }
      const screenMatrix = this.svg.getScreenCTM();
      if (screenMatrix === null) {
        return;
      }
      const screenToUserMatrix = screenMatrix.inverse();
      const svgBounds = this.svg.getBoundingClientRect();
      const clientCorner = new DOMPoint(svgBounds.right, svgBounds.top);
      const userCorner = clientCorner.matrixTransform(screenToUserMatrix);
      const userUnitsPerPixel = 1 / Math.hypot(screenMatrix.a, screenMatrix.b);
      // x = corner.x − (m + w)·u, y = corner.y + m·u, with u user units per pixel, so the elevator sits m pixels inside the top-right corner
      const x = userCorner.x - (ELEVATOR_VIEW_MARGIN_PX + this.width) * userUnitsPerPixel;
      const y = userCorner.y + ELEVATOR_VIEW_MARGIN_PX * userUnitsPerPixel;
      this.elevator.setAttribute('transform', `translate(${x} ${y}) scale(${userUnitsPerPixel})`);
    }
  }

  /** Draws the chain of floors from the root level down to the focused one; clicking a floor focuses its level. */
  class FloorChain {
    constructor(rootLabel, focusLevel) {
      this.name = 'floors';
      this.rootLabel = rootLabel;
      this.focusLevel = focusLevel;
    }

    drawInto(container, focusedLevel) {
      const levels = Contract.levelsFromRootTo(focusedLevel);
      const floors = levels.map((level, depth) => {
        const label = this.floorLabelOf(level);
        const indent = depth * ELEVATOR_FLOOR_INDENT_PX;
        const floor = ElevatorRow.create('svugy-elevator-floor', label, indent);
        const floorTop = depth * ELEVATOR_ROW_HEIGHT_PX;
        floor.setAttribute('transform', `translate(0 ${floorTop})`);
        const isFocusedFloor = level === focusedLevel;
        if (isFocusedFloor) {
          floor.classList.add('svugy-elevator-floor-current');
          return floor;
        }
        bindButton(floor, `focus ${label}`, () => this.focusLevel(level));
        return floor;
      });
      container.replaceChildren(...floors);
      const floorWidths = floors.map((floor) => ElevatorRow.naturalWidthOf(floor));
      const width = Math.max(...floorWidths);
      floors.forEach((floor) => ElevatorRow.setWidth(floor, width));
      const height = floors.length * ELEVATOR_ROW_HEIGHT_PX;
      return { width, height };
    }

    floorLabelOf(level) {
      const owner = Contract.enclosingElementOf(level);
      if (owner === null) {
        return this.rootLabel;
      }
      const ownerTitle = Contract.titleOf(owner);
      return ownerTitle ?? level.getAttribute(LEVEL);
    }
  }

  /** Draws the mini-map: the shown layer cropped to the focused level's parent, framing the focused level; clicking it focuses the parent. */
  class MiniMap {
    constructor(focusLevel) {
      this.name = 'map';
      this.focusLevel = focusLevel;
      this.miniatureSource = null;
      this.miniature = null;
    }

    drawInto(container, focusedLevel) {
      const levelsFromRoot = Contract.levelsFromRootTo(focusedLevel);
      const rootLevel = levelsFromRoot[0];
      const parentLevel = Contract.enclosingLevelOf(focusedLevel);
      const parentViewText = parentLevel.getAttribute(VIEW);
      const parentView = View.parse(parentViewText);
      const focusedViewText = focusedLevel.getAttribute(VIEW);
      const focusedView = View.parse(focusedViewText);
      const isViewMissing = parentView === null || focusedView === null;
      if (isViewMissing) {
        container.replaceChildren();
        return { width: 0, height: 0 };
      }
      // s = min(W / w, H / h): the largest scale that fits the parent's view in the map's maximum box
      const mapScale = Math.min(ELEVATOR_MAP_MAX_WIDTH_PX / parentView.width, ELEVATOR_MAP_MAX_HEIGHT_PX / parentView.height);
      const width = parentView.width * mapScale;
      const height = parentView.height * mapScale;
      const map = createSvgElement('g', { class: 'svugy-elevator-map' });
      const backdrop = createSvgElement('rect', { class: 'svugy-elevator-map-backdrop', width, height, rx: 3 });
      const viewport = createSvgElement('svg', { width, height, viewBox: parentViewText });
      const miniature = this.miniatureOf(rootLevel);
      const frame = createSvgElement('rect', {
        class: 'svugy-elevator-map-frame',
        x: focusedView.x,
        y: focusedView.y,
        width: focusedView.width,
        height: focusedView.height,
      });
      viewport.append(miniature, frame);
      map.append(backdrop, viewport);
      bindButton(map, 'focus the parent level', () => this.focusLevel(parentLevel));
      container.replaceChildren(map);
      return { width, height };
    }

    miniatureOf(rootLevel) {
      const isCachedSource = rootLevel === this.miniatureSource;
      if (isCachedSource) {
        return this.miniature;
      }
      this.miniature = Miniature.inertCloneOf(rootLevel);
      this.miniatureSource = rootLevel;
      return this.miniature;
    }
  }

  /** Clones a level into a copy that only paints: no ids, contract attributes, tab stops, links, overlays, travelers or tooltips. */
  const Miniature = {
    inertCloneOf(level) {
      const miniature = level.cloneNode(true);
      const nonPaintingNodes = miniature.querySelectorAll(NON_PAINTING_SELECTOR);
      nonPaintingNodes.forEach((node) => node.remove());
      const descendants = Array.from(miniature.querySelectorAll('*'));
      const nodes = [miniature, ...descendants];
      nodes.forEach(Miniature.stripInteractiveAttributes);
      miniature.setAttribute('class', 'svugy-elevator-miniature');
      return miniature;
    },

    stripInteractiveAttributes(node) {
      const isLink = node.localName === 'a';
      const attributeNames = node.getAttributeNames();
      const strippedNames = attributeNames.filter((name) => {
        const isContractAttribute = name.startsWith(CONTRACT_ATTRIBUTE_PREFIX);
        const isInteractiveAttribute = MINIATURE_STRIPPED_ATTRIBUTES.has(name);
        const isLinkTarget = isLink && name.endsWith('href');
        return isContractAttribute || isInteractiveAttribute || isLinkTarget;
      });
      strippedNames.forEach((name) => node.removeAttribute(name));
    },
  };

  /** Lays out one elevator row: a plate under a text label indented by `indent` pixels. */
  const ElevatorRow = {
    create(className, label, indent) {
      const row = createSvgElement('g', { class: `svugy-elevator-row ${className}` });
      const plate = createSvgElement('rect', { class: 'svugy-elevator-row-plate', rx: 4, height: ELEVATOR_ROW_HEIGHT_PX });
      const textStart = indent + ELEVATOR_LABEL_INSET_PX;
      const text = createSvgElement('text', { class: 'svugy-elevator-label', x: textStart, y: ELEVATOR_ROW_BASELINE_PX });
      text.textContent = DetailLines.truncate(label);
      row.append(plate, text);
      return row;
    },

    naturalWidthOf(row) {
      const text = row.lastElementChild;
      const textStart = Number(text.getAttribute('x'));
      const textLength = text.getComputedTextLength();
      return textStart + textLength + ELEVATOR_LABEL_INSET_PX;
    },

    setWidth(row, width) {
      const plate = row.firstElementChild;
      plate.setAttribute('width', String(width));
    },
  };

  /** Translates pointer, focus and key events into navigation, detail panels and dimming. */
  const Interactions = {
    bind(svg, navigator, panel, dimmer) {
      Interactions.makeGroupsFocusable(svg);
      svg.addEventListener('click', (event) => {
        const group = Interactions.groupAt(event.target);
        const clientPoint = { x: event.clientX, y: event.clientY };
        if (group === null) {
          Interactions.exitOneStep(navigator, panel);
          return;
        }
        Interactions.activate(group, clientPoint, navigator, panel);
      });
      svg.addEventListener('keydown', (event) => {
        const isEscape = event.key === 'Escape';
        if (isEscape) {
          Interactions.exitOneStep(navigator, panel);
          return;
        }
        const isActivationKey = event.key === 'Enter' || event.key === ' ';
        const isFocusedGroup = event.target instanceof Element && event.target.matches(INTERACTIVE_SELECTOR);
        const isGroupActivated = isActivationKey && isFocusedGroup;
        if (isGroupActivated) {
          event.preventDefault();
          const groupCenter = clientCenterOf(event.target);
          Interactions.activate(event.target, groupCenter, navigator, panel);
        }
      });
      svg.addEventListener('pointermove', (event) => {
        const group = Interactions.groupAt(event.target);
        if (group === null) {
          panel.hide();
          dimmer.clear();
          return;
        }
        const clientPoint = { x: event.clientX, y: event.clientY };
        panel.show(group, clientPoint);
        dimmer.dimUnrelatedTo(group, navigator.shownLayer);
      });
      svg.addEventListener('pointerleave', () => {
        panel.hide();
        dimmer.clear();
      });
      svg.addEventListener('focusin', (event) => {
        const group = Interactions.groupAt(event.target);
        if (group === null) {
          return;
        }
        const groupCenter = clientCenterOf(group);
        panel.show(group, groupCenter);
        dimmer.dimUnrelatedTo(group, navigator.shownLayer);
      });
      svg.addEventListener('focusout', () => {
        panel.hide();
        dimmer.clear();
      });
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
      const hasViewMoved = navigator.followClickOn(group);
      if (hasViewMoved) {
        panel.hide();
        return;
      }
      panel.show(group, clientPoint);
    },

    exitOneStep(navigator, panel) {
      panel.hide();
      navigator.exitOneStep();
    },

    groupAt(target) {
      const isElementTarget = target instanceof Element;
      return isElementTarget ? target.closest(INTERACTIVE_SELECTOR) : null;
    },

  };

  /** Makes a runtime-built node act as a button: focusable, labelled, and activated by a click, Enter or Space. */
  function bindButton(node, label, action) {
    node.setAttribute('role', 'button');
    node.setAttribute('tabindex', '0');
    node.setAttribute('aria-label', label);
    node.addEventListener('click', () => action());
    node.addEventListener('keydown', (event) => {
      const isActivationKey = event.key === 'Enter' || event.key === ' ';
      if (isActivationKey) {
        event.preventDefault();
        action();
      }
    });
  }

  function clientCenterOf(node) {
    const bounds = node.getBoundingClientRect();
    return { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 };
  }

  function createSvgElement(name, attributes) {
    const element = document.createElementNS(SVG_NAMESPACE, name);
    Object.entries(attributes).forEach(([attribute, value]) => element.setAttribute(attribute, String(value)));
    return element;
  }
  start();
})();
