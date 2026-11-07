(() => {
  'use strict';

  const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';
  const XHTML_NAMESPACE = 'http://www.w3.org/1999/xhtml';

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
  const DETAILS = 'data-svugy-details';
  const HEADING = 'data-svugy-heading';
  const ENTERED = 'data-svugy-entered';
  const DIMMED = 'data-svugy-dimmed';

  const LAYER_SELECTOR = `[${LAYER}]`;
  const LEVEL_SELECTOR = `[${LEVEL}]`;
  const ELEMENT_SELECTOR = `[${ELEMENT}]`;
  const EDGE_SELECTOR = `[${EDGE}]`;
  const SHOWN_SELECTOR = `[${SHOWN}]`;
  const ENTERED_SELECTOR = `[${ENTERED}]`;
  const DIMMED_SELECTOR = `[${DIMMED}]`;
  const INTERACTIVE_SELECTOR = `[${ELEMENT}],[${EDGE}],[${STUB}]`;
  const HEADING_SELECTOR = `[${HEADING}]`;
  const ELEMENT_OR_LEVEL_SELECTOR = `[${ELEMENT}],[${LEVEL}]`;

  const ROOT_LEVEL_PATH = '';
  const LEVEL_LINK_PREFIX = '#svugy=';
  const LEVEL_LINK_PATH_KEY = 'svugy';
  const LEVEL_LINK_LAYER_KEY = 'layer';
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

  const DESCRIPTION_WIDTH_PX = 320;
  const DESCRIPTION_VIEW_MARGIN_PX = 12;
  const LIFTED_KIND = 'lifted';

  const PARAGRAPH_BLOCK = 'paragraph';
  const HEADING_BLOCK = 'heading';
  const CODE_BLOCK = 'code';
  const LIST_BLOCK = 'list';
  const FENCE = '```';
  const HEADING_PATTERN = /^(#{1,3})\s+(.*)$/;
  const LIST_ITEM_PATTERN = /^(\s*)([-*]|\d+\.)\s+(.*)$/;
  const ORDERED_MARKER_PATTERN = /^\d/;
  const INDENTED_LINE_PATTERN = /^\s/;
  const LINE_BREAK_PATTERN = /\r?\n/;
  const AUTHOR_HEADING_RANK_OFFSET = 3;
  const INLINE_PATTERN = /`([^`]+)`|\*\*(.+?)\*\*|\*([^*\s][^*]*)\*|\[([^\]]+)\]\(([^()\s]+)\)/g;
  const CONTROL_OR_SPACE_PATTERN = /[\u0000-\u0020\u007f]/;
  const HTTP_SCHEME_PATTERN = /^https?:/i;
  const ANY_SCHEME_PATTERN = /^[^/?#]*:/;
  const TARGET_LINK_PREFIX = '#';
  const EDGE_TARGET_PATTERN = /^(.+->.+)\[(\d+)\]$/;
  const HEX_COLOR_PATTERN = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i;
  const RGB_COLOR_PATTERN = /^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)/i;
  const FLOW_PATH_SELECTOR = 'path[data-svugy-flow]';
  const LINK_HOVER_PROPERTY = '--svugy-link-hover';
  const MIN_LINK_CONTRAST = 3;

  const HIGHLIGHTED_CLASS = 'svugy-highlighted';
  const HIGHLIGHT_DURATION_MS = 1200;

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

  /** Reads the level path and layer index a `#svugy=PATH&layer=K` fragment of the document's URL links to, PATH URL-encoded, `&layer=K` optional; null without one. */
  function levelLink() {
    const fragment = window.location.hash;
    const isOtherFragment = !fragment.startsWith(LEVEL_LINK_PREFIX);
    if (isOtherFragment) {
      return null;
    }
    const fragmentQuery = fragment.slice(1);
    const fragmentParameters = new URLSearchParams(fragmentQuery);
    const path = fragmentParameters.get(LEVEL_LINK_PATH_KEY);
    const layerText = fragmentParameters.get(LEVEL_LINK_LAYER_KEY);
    const layerIndex = layerText === null ? 0 : Number(layerText);
    return { path, layerIndex };
  }

  /** Wires navigation, the detail and description panels and dimming onto one diagram. */
  function bindRuntime(svg) {
    const layers = Layers.of(svg);
    const initialLayer = layers[0];
    const deepestLayer = layers[layers.length - 1];
    const rootLevel = Contract.levelAt(initialLayer, ROOT_LEVEL_PATH);
    if (rootLevel === null) {
      return;
    }
    const animator = new ViewBoxAnimator(svg);
    const fader = new LayerFader();
    const navigator = new LayerNavigator(svg, layers, initialLayer, rootLevel, animator, fader);
    const panel = new DetailPanel(svg);
    const dimmer = new UnrelatedGroupDimmer(svg);
    const descriptionPanel = new DescriptionPanel(svg, deepestLayer);
    const highlighter = new TargetHighlighter();
    navigator.addNavigationListener((focusedLevel) => {
      panel.hide();
      dimmer.clear();
      const focusedPath = focusedLevel.getAttribute(LEVEL);
      descriptionPanel.describeFocusedLevel(focusedPath);
    });
    descriptionPanel.describeFocusedLevel(ROOT_LEVEL_PATH);
    Interactions.bind(svg, navigator, panel, dimmer, descriptionPanel, highlighter);
    const link = levelLink();
    if (link !== null) {
      navigator.jumpTo(link.path, link.layerIndex);
    }
  }

  /** Lists the layers a document draws, in increasing depth; a document without layer groups is its own single layer. */
  const Layers = {
    of(svg) {
      const layerGroups = Array.from(svg.querySelectorAll(LAYER_SELECTOR));
      const hasLayerGroups = layerGroups.length > 0;
      return hasLayerGroups ? layerGroups : [svg];
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

    edgesAt(scope, edgeName) {
      const edges = Array.from(scope.querySelectorAll(EDGE_SELECTOR));
      return edges.filter((edge) => edge.getAttribute(EDGE) === edgeName);
    },

    /** Finds the written-edge or element group of `scope` a description link's canonical target names: `#FROM->TO[n]`, else `#PATH`; null when it names nothing. */
    groupTargetedBy(scope, target) {
      const targetName = target.slice(TARGET_LINK_PREFIX.length);
      const edgeTarget = EDGE_TARGET_PATTERN.exec(targetName);
      if (edgeTarget === null) {
        return Contract.elementAt(scope, targetName);
      }
      const [_targetName, endpointsText, ordinal] = edgeTarget;
      const edges = Array.from(scope.querySelectorAll(EDGE_SELECTOR));
      const targetedEdge = edges.find((edge) => {
        const [from, to] = Contract.endpointsOf(edge);
        const isSameEndpoints = `${from}->${to}` === endpointsText;
        const writtenEdgeName = `(${from} -> ${to})[${ordinal}]`;
        const isSameOrdinal = edge.getAttribute(EDGE) === writtenEdgeName;
        return isSameEndpoints && isSameOrdinal;
      });
      return targetedEdge ?? null;
    },

    /** Lists the groups of `scope` drawing the element or edge `group` draws: the element's group, or every group of the edge, its label's included. */
    groupsDrawing(scope, group) {
      const elementPath = group.getAttribute(ELEMENT);
      if (elementPath === null) {
        const edgeName = group.getAttribute(EDGE);
        return Contract.edgesAt(scope, edgeName);
      }
      const element = Contract.elementAt(scope, elementPath);
      return element === null ? [] : [element];
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
      let depth = 0;
      for (let ancestor = Contract.enclosingLevelOf(level); ancestor !== null; ancestor = Contract.enclosingLevelOf(ancestor)) {
        depth += 1;
      }
      return depth;
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

    // width = min(max(w, h·A), canvas width), height = width / A, A the canvas aspect; the view centred, then clamped into the canvas
    fittedInside(view, canvas) {
      const canvasAspect = canvas.width / canvas.height;
      const aspectWidth = Math.max(view.width, view.height * canvasAspect);
      const width = Math.min(aspectWidth, canvas.width);
      const height = width / canvasAspect;
      const centredX = view.x + (view.width - width) / 2;
      const centredY = view.y + (view.height - height) / 2;
      const rightmostX = canvas.x + canvas.width - width;
      const lowestY = canvas.y + canvas.height - height;
      const x = Math.max(canvas.x, Math.min(centredX, rightmostX));
      const y = Math.max(canvas.y, Math.min(centredY, lowestY));
      return { x, y, width, height };
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

  /** Tweens or jumps the root viewBox to a target view fitted to the canvas, so no letterbox ever falls outside the canvas; tweens jump under reduced motion. */
  class ViewBoxAnimator {
    constructor(svg) {
      this.svg = svg;
      this.frameRequest = 0;
      this.canvasView = this.currentView();
    }

    tweenTo(requestedView, startView = this.currentView()) {
      const isMotionReduced = window.matchMedia(REDUCED_MOTION_QUERY).matches;
      if (isMotionReduced) {
        this.jumpTo(requestedView);
        return;
      }
      cancelAnimationFrame(this.frameRequest);
      const targetView = View.fittedInside(requestedView, this.canvasView);
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

    jumpTo(requestedView) {
      cancelAnimationFrame(this.frameRequest);
      const targetView = View.fittedInside(requestedView, this.canvasView);
      this.show(targetView);
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

    /** Shows layer `layerIndex` focused on level `path`, without animation, or the first layer holding that level when layer `layerIndex` does not; a path that is no level changes nothing. */
    jumpTo(path, layerIndex) {
      const requestedLayer = this.layers[layerIndex];
      const levelInRequestedLayer = requestedLayer === undefined ? null : Contract.levelAt(requestedLayer, path);
      const isLevelInRequestedLayer = levelInRequestedLayer !== null;
      const firstOpeningLayer = this.layers.find((layer) => Contract.levelAt(layer, path) !== null);
      const openingLayer = isLevelInRequestedLayer ? requestedLayer : firstOpeningLayer;
      if (openingLayer === undefined) {
        return;
      }
      const linkedLevel = Contract.levelAt(openingLayer, path);
      this.shownLayer.removeAttribute(SHOWN);
      openingLayer.setAttribute(SHOWN, '');
      this.shownLayer = openingLayer;
      this.moveEnteredMarkTo(linkedLevel);
      this.announceNavigation();
      const targetView = this.viewOf(linkedLevel);
      if (targetView !== null) {
        this.animator.jumpTo(targetView);
      }
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

    /** Shows `target`, an element or edge group of the deepest layer, where a description link reads it best: a container as the focused level, a leaf or a badge in its enclosing level, an arc in its home level; each in the first layer from the shown one drawing that level open, the edge as written. */
    revealGroup(target) {
      const interiorLevel = Contract.interiorLevelOf(target);
      const readingLevel = interiorLevel ?? Contract.enclosingLevelOf(target);
      const readingPath = readingLevel.getAttribute(LEVEL);
      const isEdge = target.hasAttribute(EDGE);
      if (isEdge) {
        const edgeName = target.getAttribute(EDGE);
        const writingLayer = this.firstLayerFromShownWhere((layer) => Contract.edgesAt(layer, edgeName).length > 0);
        if (writingLayer === null) {
          return;
        }
        this.focusLevelIn(writingLayer, readingPath, this.focusedLevel);
        return;
      }
      const shownCounterpart = Contract.counterpartIn(this.shownLayer, target);
      const anchor = shownCounterpart ?? this.focusedLevel;
      this.goTo(readingPath, anchor);
    }

    goTo(path, group) {
      const openingLayer = this.firstLayerFromShownWhere((layer) => Contract.levelAt(layer, path) !== null);
      if (openingLayer === null) {
        const displayingLevel = Contract.levelDisplaying(this.shownLayer, path);
        if (displayingLevel === null) {
          return false;
        }
        return this.focus(displayingLevel);
      }
      return this.focusLevelIn(openingLayer, path, group);
    }

    firstLayerFromShownWhere(isMatchingLayer) {
      const shownLayerIndex = this.layers.indexOf(this.shownLayer);
      const layersFromShown = this.layers.slice(shownLayerIndex);
      const matchingLayer = layersFromShown.find(isMatchingLayer);
      return matchingLayer ?? null;
    }

    /** Focuses level `path` of `layer`, crossfading to `layer` around `anchor` when it is not the shown one. */
    focusLevelIn(layer, path, anchor) {
      const isShownLayer = layer === this.shownLayer;
      if (isShownLayer) {
        const level = Contract.levelAt(layer, path);
        return this.focus(level);
      }
      return this.showLayer(layer, path, anchor);
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

  /** Shows the author's description of the focused level, or of the last clicked group or heading, in a panel floating top-right at a constant on-screen width; nothing without a description. */
  class DescriptionPanel {
    constructor(svg, deepestLayer) {
      this.svg = svg;
      this.deepestLayer = deepestLayer;
      this.body = createXhtmlElement('aside', { class: 'svugy-description', 'aria-label': 'description' });
      this.frame = createSvgElement('foreignObject', { class: 'svugy-description-frame' });
      this.frame.appendChild(this.body);
      this.panel = createSvgElement('g', { class: 'svugy-description-panel' });
      this.panel.appendChild(this.frame);
      this.panel.addEventListener('click', (event) => event.stopPropagation());
      this.focusedPath = ROOT_LEVEL_PATH;
      this.isClickShown = false;
      this.isShownContentKept = false;
      this.width = 0;
      this.targetLinkListeners = [];
      const markdown = new SafeMarkdown(deepestLayer, (target) => this.announceTargetLink(target));
      this.content = new DescriptionContent(deepestLayer, markdown);
      const viewBoxObserver = new MutationObserver(() => this.place());
      viewBoxObserver.observe(svg, { attributes: true, attributeFilter: ['viewBox'] });
      const resizeObserver = new ResizeObserver(() => this.layOut());
      resizeObserver.observe(svg);
    }

    /** Calls `listener(target)` when a description link to `target`, an element or edge group of the deepest layer, is activated. */
    addTargetLinkListener(listener) {
      this.targetLinkListeners.push(listener);
    }

    /** Runs `navigate` without replacing the shown description, so a link's reader keeps reading while the view moves to its target. */
    keepShownContentWhile(navigate) {
      this.isShownContentKept = true;
      navigate();
      this.isShownContentKept = false;
    }

    describeFocusedLevel(path) {
      this.focusedPath = path;
      if (this.isShownContentKept) {
        this.isClickShown = true;
        return;
      }
      this.isClickShown = false;
      const focusedContent = this.contentOfFocusedLevel();
      this.show(focusedContent);
    }

    /** Describes a clicked group: the focused level's own description when the group is that level's container, else the group's. */
    describeClickedGroup(group) {
      const groupPath = group.getAttribute(ELEMENT);
      const isFocusedContainer = groupPath === this.focusedPath;
      if (isFocusedContainer) {
        this.describeFocusedLevel(this.focusedPath);
        return;
      }
      const groupContent = this.content.of(group);
      this.showClicked(groupContent);
    }

    describeClickedHeading() {
      const diagramContent = this.content.of(this.svg);
      this.showClicked(diagramContent);
    }

    /** Goes back from a clicked description to the focused level's and tells whether that changed the panel; a click that closed the panel over an undescribed level changed nothing to go back from. */
    stepBackFromClick() {
      const isFocusedLevelShown = !this.isClickShown;
      if (isFocusedLevelShown) {
        return false;
      }
      this.isClickShown = false;
      const focusedContent = this.contentOfFocusedLevel();
      const isPanelClosed = !this.panel.isConnected;
      const isFocusedLevelUndescribed = focusedContent.length === 0;
      const isPanelUnchanged = isPanelClosed && isFocusedLevelUndescribed;
      if (isPanelUnchanged) {
        return false;
      }
      this.show(focusedContent);
      return true;
    }

    contentOfFocusedLevel() {
      const isRoot = this.focusedPath === ROOT_LEVEL_PATH;
      const describedNode = isRoot ? this.svg : Contract.elementAt(this.deepestLayer, this.focusedPath);
      return describedNode === null ? [] : this.content.of(describedNode);
    }

    showClicked(content) {
      this.isClickShown = true;
      this.show(content);
    }

    show(content) {
      const wasFocusInside = this.panel.contains(document.activeElement);
      this.body.replaceChildren(...content);
      if (wasFocusInside) {
        this.svg.focus({ preventScroll: true });
      }
      const isContentless = content.length === 0;
      if (isContentless) {
        this.panel.remove();
        return;
      }
      const isDetached = !this.panel.isConnected;
      if (isDetached) {
        this.svg.appendChild(this.panel);
      }
      this.layOut();
      this.body.scrollTop = 0;
    }

    /** Sizes and places the shown panel. */
    layOut() {
      const isDetached = !this.panel.isConnected;
      if (isDetached) {
        return;
      }
      this.fit();
      this.place();
    }

    // w = min(W, client width − 2m), h = min(content height, client height − 2m): the panel never leaves the viewport
    fit() {
      const svgBounds = this.svg.getBoundingClientRect();
      const viewportMargins = 2 * DESCRIPTION_VIEW_MARGIN_PX;
      const availableWidth = Math.max(0, svgBounds.width - viewportMargins);
      const availableHeight = Math.max(0, svgBounds.height - viewportMargins);
      this.width = Math.min(DESCRIPTION_WIDTH_PX, availableWidth);
      this.frame.setAttribute('width', String(this.width));
      this.frame.setAttribute('height', String(availableHeight));
      const contentHeight = this.body.offsetHeight;
      this.frame.setAttribute('height', String(contentHeight));
    }

    place() {
      const isDetached = !this.panel.isConnected;
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
      // x = corner.x − (m + w)·u, y = corner.y + m·u, with u user units per pixel, so the panel sits m pixels inside the top-right corner
      const x = userCorner.x - (DESCRIPTION_VIEW_MARGIN_PX + this.width) * userUnitsPerPixel;
      const y = userCorner.y + DESCRIPTION_VIEW_MARGIN_PX * userUnitsPerPixel;
      this.panel.setAttribute('transform', `translate(${x} ${y}) scale(${userUnitsPerPixel})`);
    }

    announceTargetLink(target) {
      this.targetLinkListeners.forEach((listener) => listener(target));
    }
  }

  /** Builds the description panel's XHTML from a group's or the diagram's details: its description as Markdown, or for a lifted edge each described source edge's description under that edge's name. */
  class DescriptionContent {
    constructor(deepestLayer, markdown) {
      this.deepestLayer = deepestLayer;
      this.markdown = markdown;
    }

    of(node) {
      const details = Contract.detailsOf(node);
      const isLifted = details?.kind === LIFTED_KIND;
      return isLifted ? this.ofLiftedEdge(details) : this.descriptionBlocksOf(details);
    }

    ofLiftedEdge(details) {
      const sourceEdges = DetailLines.plainObjectsIn(details.edges);
      const describedSourceEdges = sourceEdges.filter(DescriptionContent.isDescribed);
      return describedSourceEdges.map((sourceEdge) => this.sourceEdgeSectionOf(sourceEdge));
    }

    /** A section headed by the source edge's label, else its endpoints' names joined by an arrow, then its description. */
    sourceEdgeSectionOf(sourceEdge) {
      const fromName = this.nameOfElementAt(sourceEdge.from);
      const toName = this.nameOfElementAt(sourceEdge.to);
      const isLabelled = typeof sourceEdge.label === 'string';
      const headingText = isLabelled ? sourceEdge.label : `${fromName} ${ARROW} ${toName}`;
      const heading = createXhtmlElement('h3', { class: 'svugy-description-source-edge' });
      heading.textContent = headingText;
      const descriptionNodes = this.markdown.blocksOf(sourceEdge.description);
      const section = createXhtmlElement('section', {});
      section.append(heading, ...descriptionNodes);
      return section;
    }

    descriptionBlocksOf(details) {
      const isDescribed = DescriptionContent.isDescribed(details);
      return isDescribed ? this.markdown.blocksOf(details.description) : [];
    }

    static isDescribed(details) {
      const description = details?.description;
      return typeof description === 'string';
    }

    nameOfElementAt(path) {
      const element = Contract.elementAt(this.deepestLayer, path);
      const title = element === null ? null : Contract.titleOf(element);
      return title ?? path;
    }
  }

  /** Renders the panel's Markdown subset as XHTML nodes: paragraphs keeping their line breaks, `-`/`*` and `1.` lists nested one level, `#`–`###` headings as h4–h6, fenced code, `**bold**`, `*em*`, `` `code` ``, `[text](url)` links to http(s) or relative URLs, and `[text](#target)` buttons following a link to a diagram object; author text only ever becomes text nodes, never markup. */
  /** Picks the colour a description link takes on hover: its element's top face or its edge's stroke, when that colour reads against the canvas. */
  const TargetColor = {
    readableHoverColorOf(group) {
      const isEdge = group.hasAttribute(EDGE);
      const paint = isEdge ? TargetColor.flowStrokeOf(group) : TargetColor.topFaceFillOf(group);
      if (paint === null) {
        return null;
      }
      const svg = group.ownerSVGElement;
      const svgStyle = getComputedStyle(svg);
      const canvasPaint = svgStyle.getPropertyValue('--svugy-canvas').trim();
      const paintChannels = TargetColor.channelsOf(paint);
      const canvasChannels = TargetColor.channelsOf(canvasPaint);
      const isEitherUnparsed = paintChannels === null || canvasChannels === null;
      if (isEitherUnparsed) {
        return null;
      }
      const contrast = TargetColor.contrastBetween(paintChannels, canvasChannels);
      const isReadable = contrast >= MIN_LINK_CONTRAST;
      return isReadable ? paint : null;
    },

    flowStrokeOf(group) {
      const flowPath = group.querySelector(FLOW_PATH_SELECTOR);
      if (flowPath === null) {
        return null;
      }
      const flowStyle = getComputedStyle(flowPath);
      return flowStyle.stroke;
    },

    topFaceFillOf(group) {
      const faces = Array.from(group.querySelectorAll(':scope > path'));
      const filledFaces = faces.filter((face) => face.getAttribute('fill') !== 'none');
      const topFace = filledFaces.at(-1);
      if (topFace === undefined) {
        return null;
      }
      const topFaceStyle = getComputedStyle(topFace);
      return topFaceStyle.fill;
    },

    channelsOf(paint) {
      const hexMatch = HEX_COLOR_PATTERN.exec(paint);
      if (hexMatch !== null) {
        const hexDigits = hexMatch[1];
        const isShortHex = hexDigits.length === 3;
        const fullHexDigits = isShortHex ? hexDigits.replace(/./g, '$&$&') : hexDigits;
        const channelPairs = fullHexDigits.match(/../g);
        return channelPairs.map((pair) => parseInt(pair, 16));
      }
      const rgbMatch = RGB_COLOR_PATTERN.exec(paint);
      if (rgbMatch === null) {
        return null;
      }
      const [_rgbText, red, green, blue] = rgbMatch;
      return [red, green, blue].map(Number);
    },

    // contrast = (L_light + 0.05) / (L_dark + 0.05), L = 0.2126·R + 0.7152·G + 0.0722·B over linearised channels (WCAG 2)
    contrastBetween(firstChannels, secondChannels) {
      const firstLuminance = TargetColor.relativeLuminanceOf(firstChannels);
      const secondLuminance = TargetColor.relativeLuminanceOf(secondChannels);
      const lighterLuminance = Math.max(firstLuminance, secondLuminance);
      const darkerLuminance = Math.min(firstLuminance, secondLuminance);
      return (lighterLuminance + 0.05) / (darkerLuminance + 0.05);
    },

    // c = v/255; linear = c ≤ 0.03928 ? c/12.92 : ((c + 0.055)/1.055)^2.4
    relativeLuminanceOf(channels) {
      const [red, green, blue] = channels.map((channel) => {
        const unitChannel = channel / 255;
        const isDarkChannel = unitChannel <= 0.03928;
        return isDarkChannel ? unitChannel / 12.92 : ((unitChannel + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
    },
  };

  class SafeMarkdown {
    constructor(deepestLayer, followTargetLink) {
      this.deepestLayer = deepestLayer;
      this.followTargetLink = followTargetLink;
    }

    blocksOf(markdown) {
      const blocks = MarkdownBlocks.of(markdown);
      return blocks.map((block) => this.nodeOfBlock(block));
    }

    nodeOfBlock(block) {
      const nodeOfKind = NODE_OF_BLOCK_BY_KIND.get(block.kind);
      return nodeOfKind.call(this, block);
    }

    paragraphNodeOf(paragraph) {
      return this.elementHoldingLines('p', paragraph.lines);
    }

    headingNodeOf(heading) {
      const rank = heading.level + AUTHOR_HEADING_RANK_OFFSET;
      return this.elementHoldingLines(`h${rank}`, [heading.text]);
    }

    codeNodeOf(codeBlock) {
      const code = createXhtmlElement('code', {});
      code.textContent = codeBlock.lines.join('\n');
      const preformatted = createXhtmlElement('pre', {});
      preformatted.appendChild(code);
      return preformatted;
    }

    listNodeOf(list) {
      const listName = list.isOrdered ? 'ol' : 'ul';
      const listNode = createXhtmlElement(listName, {});
      const isStartingPastOne = list.isOrdered && list.start !== 1;
      if (isStartingPastOne) {
        listNode.setAttribute('start', String(list.start));
      }
      const itemNodes = list.items.map((item) => this.listItemNodeOf(item));
      listNode.append(...itemNodes);
      return listNode;
    }

    listItemNodeOf(item) {
      const itemNode = this.elementHoldingLines('li', item.lines);
      const nestedListNodes = item.lists.map((nestedList) => this.listNodeOf(nestedList));
      itemNode.append(...nestedListNodes);
      return itemNode;
    }

    elementHoldingLines(name, lines) {
      const element = createXhtmlElement(name, {});
      lines.forEach((line, index) => {
        const isLaterLine = index > 0;
        if (isLaterLine) {
          const lineBreak = createXhtmlElement('br', {});
          element.appendChild(lineBreak);
        }
        const inlineNodes = this.inlineNodesOf(line);
        element.append(...inlineNodes);
      });
      return element;
    }

    inlineNodesOf(text) {
      const nodes = [];
      let consumedLength = 0;
      for (const match of text.matchAll(INLINE_PATTERN)) {
        const precedingText = text.slice(consumedLength, match.index);
        const precedingNode = document.createTextNode(precedingText);
        const matchNode = this.inlineNodeOf(match);
        nodes.push(precedingNode, matchNode);
        consumedLength = match.index + match[0].length;
      }
      const trailingText = text.slice(consumedLength);
      const trailingNode = document.createTextNode(trailingText);
      nodes.push(trailingNode);
      return nodes;
    }

    inlineNodeOf(match) {
      const [_source, code, bold, emphasis, linkText, linkUrl] = match;
      if (code !== undefined) {
        const codeNode = createXhtmlElement('code', {});
        codeNode.textContent = code;
        return codeNode;
      }
      if (bold !== undefined) {
        return this.elementHoldingLines('strong', [bold]);
      }
      if (emphasis !== undefined) {
        return this.elementHoldingLines('em', [emphasis]);
      }
      const isTargetLink = linkUrl.startsWith(TARGET_LINK_PREFIX);
      if (isTargetLink) {
        return this.targetLinkNodeOf(linkText, linkUrl);
      }
      const isSafeUrl = SafeMarkdown.isSafeUrl(linkUrl);
      if (isSafeUrl) {
        const link = this.elementHoldingLines('a', [linkText]);
        link.setAttribute('href', linkUrl);
        link.setAttribute('target', '_blank');
        link.setAttribute('rel', 'noopener noreferrer');
        return link;
      }
      return this.elementHoldingLines('span', [linkText]);
    }

    /** A button following the link to the element or edge `target` names, or the link's text alone when it names nothing in the diagram. */
    targetLinkNodeOf(linkText, target) {
      const targetedGroup = Contract.groupTargetedBy(this.deepestLayer, target);
      if (targetedGroup === null) {
        return this.elementHoldingLines('span', [linkText]);
      }
      const button = this.elementHoldingLines('button', [linkText]);
      button.setAttribute('type', 'button');
      button.addEventListener('click', () => this.followTargetLink(targetedGroup));
      const hoverColor = TargetColor.readableHoverColorOf(targetedGroup);
      if (hoverColor !== null) {
        button.style.setProperty(LINK_HOVER_PROPERTY, hoverColor);
      }
      return button;
    }

    // safe = no control or space character, and an http(s) scheme or none at all: no ':' before the first '/', '?' or '#'
    static isSafeUrl(url) {
      const hasControlOrSpace = CONTROL_OR_SPACE_PATTERN.test(url);
      if (hasControlOrSpace) {
        return false;
      }
      const isHttp = HTTP_SCHEME_PATTERN.test(url);
      const hasScheme = ANY_SCHEME_PATTERN.test(url);
      return isHttp || !hasScheme;
    }
  }

  const NODE_OF_BLOCK_BY_KIND = new Map([
    [PARAGRAPH_BLOCK, SafeMarkdown.prototype.paragraphNodeOf],
    [HEADING_BLOCK, SafeMarkdown.prototype.headingNodeOf],
    [CODE_BLOCK, SafeMarkdown.prototype.codeNodeOf],
    [LIST_BLOCK, SafeMarkdown.prototype.listNodeOf],
  ]);

  /** Splits Markdown into blocks: paragraphs of lines, `#`–`###` headings, fenced code, and `-`/`*` or `1.` lists whose indented items nest one level under the item above. */
  class MarkdownBlocks {
    static of(markdown) {
      const reader = new MarkdownBlocks();
      const lines = markdown.split(LINE_BREAK_PATTERN);
      lines.forEach((line) => reader.read(line));
      return reader.blocks;
    }

    constructor() {
      this.blocks = [];
      this.openCode = null;
      this.openParagraph = null;
      this.openItem = null;
    }

    // inside a fence every line is code up to the closing fence; else a blank line closes the open paragraph and item, an indented line continues the open item, any other line continues the open paragraph or starts one
    read(line) {
      if (this.openCode !== null) {
        this.readCodeLine(line);
        return;
      }
      const trimmedLine = line.trim();
      const isBlank = trimmedLine === '';
      if (isBlank) {
        this.openParagraph = null;
        this.openItem = null;
        return;
      }
      const isFence = trimmedLine.startsWith(FENCE);
      if (isFence) {
        const codeBlock = { kind: CODE_BLOCK, lines: [] };
        this.addBlock(codeBlock);
        this.openCode = codeBlock;
        return;
      }
      const heading = HEADING_PATTERN.exec(trimmedLine);
      if (heading !== null) {
        const [_line, hashes, text] = heading;
        const headingBlock = { kind: HEADING_BLOCK, level: hashes.length, text };
        this.addBlock(headingBlock);
        return;
      }
      const listItem = LIST_ITEM_PATTERN.exec(line);
      if (listItem !== null) {
        this.addListItem(listItem);
        return;
      }
      const isIndented = INDENTED_LINE_PATTERN.test(line);
      const isItemContinuation = isIndented && this.openItem !== null;
      if (isItemContinuation) {
        this.openItem.lines.push(trimmedLine);
        return;
      }
      if (this.openParagraph !== null) {
        this.openParagraph.lines.push(trimmedLine);
        return;
      }
      const paragraph = { kind: PARAGRAPH_BLOCK, lines: [trimmedLine] };
      this.addBlock(paragraph);
      this.openParagraph = paragraph;
    }

    readCodeLine(line) {
      const trimmedLine = line.trim();
      const isClosingFence = trimmedLine.startsWith(FENCE);
      if (isClosingFence) {
        this.openCode = null;
        return;
      }
      this.openCode.lines.push(line);
    }

    addBlock(block) {
      this.openParagraph = null;
      this.openItem = null;
      this.blocks.push(block);
    }

    // an indented item nests under the last item of a list ending the blocks; any other item joins or starts a top-level list
    addListItem(listItem) {
      const [_line, indentation, marker, text] = listItem;
      const isOrdered = ORDERED_MARKER_PATTERN.test(marker);
      const start = Number.parseInt(marker, 10);
      const item = { lines: [text], lists: [] };
      const lastBlock = this.blocks.at(-1);
      const isListLast = lastBlock?.kind === LIST_BLOCK;
      const parentItem = isListLast ? lastBlock.items.at(-1) : undefined;
      const isIndentedItem = indentation !== '';
      const isNestedItem = isIndentedItem && parentItem !== undefined;
      const siblingLists = isNestedItem ? parentItem.lists : this.blocks;
      const list = MarkdownBlocks.listToJoinIn(siblingLists, isOrdered, start);
      list.items.push(item);
      this.openParagraph = null;
      this.openItem = item;
    }

    /** Returns the last of `blocks` when it is a list of this kind, else a new such list appended to `blocks`. */
    static listToJoinIn(blocks, isOrdered, start) {
      const lastBlock = blocks.at(-1);
      const isListOfKind = lastBlock?.kind === LIST_BLOCK && lastBlock.isOrdered === isOrdered;
      if (isListOfKind) {
        return lastBlock;
      }
      const list = { kind: LIST_BLOCK, isOrdered, start, items: [] };
      blocks.push(list);
      return list;
    }
  }

  /** Highlights the groups a description link targets for HIGHLIGHT_DURATION_MS, ending any earlier highlight first. */
  class TargetHighlighter {
    constructor() {
      this.highlightedGroups = [];
      this.expiryTimer = 0;
    }

    highlight(groups) {
      this.clear();
      groups.forEach((group) => group.classList.add(HIGHLIGHTED_CLASS));
      this.highlightedGroups = groups;
      this.expiryTimer = setTimeout(() => this.clear(), HIGHLIGHT_DURATION_MS);
    }

    clear() {
      clearTimeout(this.expiryTimer);
      this.highlightedGroups.forEach((group) => group.classList.remove(HIGHLIGHTED_CLASS));
      this.highlightedGroups = [];
    }
  }

  /** Translates pointer, focus and key events and description links into navigation, detail and description panels, dimming and highlights. */
  const Interactions = {
    bind(svg, navigator, panel, dimmer, descriptionPanel, highlighter) {
      Interactions.makeGroupsFocusable(svg);
      descriptionPanel.addTargetLinkListener((target) => {
        Interactions.followTargetLink(target, navigator, panel, descriptionPanel, highlighter);
      });
      svg.addEventListener('click', (event) => {
        const heading = Interactions.headingAt(event.target);
        if (heading !== null) {
          descriptionPanel.describeClickedHeading();
          return;
        }
        const group = Interactions.groupAt(event.target);
        const clientPoint = { x: event.clientX, y: event.clientY };
        if (group === null) {
          Interactions.exitOneStep(navigator, panel, descriptionPanel);
          return;
        }
        Interactions.activate(group, clientPoint, navigator, panel, descriptionPanel);
      });
      svg.addEventListener('keydown', (event) => {
        const isEscape = event.key === 'Escape';
        if (isEscape) {
          Interactions.exitOneStep(navigator, panel, descriptionPanel);
          return;
        }
        const isActivationKey = event.key === 'Enter' || event.key === ' ';
        const isElementTarget = event.target instanceof Element;
        const isFocusedHeading = isElementTarget && event.target.matches(HEADING_SELECTOR);
        const isHeadingActivated = isActivationKey && isFocusedHeading;
        if (isHeadingActivated) {
          event.preventDefault();
          descriptionPanel.describeClickedHeading();
          return;
        }
        const isFocusedGroup = isElementTarget && event.target.matches(INTERACTIVE_SELECTOR);
        const isGroupActivated = isActivationKey && isFocusedGroup;
        if (isGroupActivated) {
          event.preventDefault();
          const groupCenter = clientCenterOf(event.target);
          Interactions.activate(event.target, groupCenter, navigator, panel, descriptionPanel);
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
        group.setAttribute('role', 'button');
        const title = Contract.titleOf(group);
        if (title !== null) {
          group.setAttribute('aria-label', title);
        }
      });
      const headings = svg.querySelectorAll(HEADING_SELECTOR);
      headings.forEach((heading) => {
        heading.setAttribute('tabindex', '0');
        heading.setAttribute('role', 'button');
        heading.setAttribute('aria-label', 'diagram description');
      });
    },

    activate(group, clientPoint, navigator, panel, descriptionPanel) {
      const hasViewMoved = navigator.followClickOn(group);
      if (hasViewMoved) {
        panel.hide();
      } else {
        panel.show(group, clientPoint);
      }
      const isJumpElsewhere = group.hasAttribute(GOTO);
      if (isJumpElsewhere) {
        return;
      }
      descriptionPanel.describeClickedGroup(group);
    },

    followTargetLink(target, navigator, panel, descriptionPanel, highlighter) {
      panel.hide();
      descriptionPanel.keepShownContentWhile(() => navigator.revealGroup(target));
      const shownGroups = Contract.groupsDrawing(navigator.shownLayer, target);
      highlighter.highlight(shownGroups);
    },

    exitOneStep(navigator, panel, descriptionPanel) {
      panel.hide();
      const hasLeftClick = descriptionPanel.stepBackFromClick();
      if (hasLeftClick) {
        return;
      }
      navigator.exitOneStep();
    },

    groupAt(target) {
      const isElementTarget = target instanceof Element;
      return isElementTarget ? target.closest(INTERACTIVE_SELECTOR) : null;
    },

    headingAt(target) {
      const isElementTarget = target instanceof Element;
      return isElementTarget ? target.closest(HEADING_SELECTOR) : null;
    },

  };

  function clientCenterOf(node) {
    const bounds = node.getBoundingClientRect();
    return { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 };
  }

  function createSvgElement(name, attributes) {
    return createElementIn(SVG_NAMESPACE, name, attributes);
  }

  function createXhtmlElement(name, attributes) {
    return createElementIn(XHTML_NAMESPACE, name, attributes);
  }

  function createElementIn(namespace, name, attributes) {
    const element = document.createElementNS(namespace, name);
    Object.entries(attributes).forEach(([attribute, value]) => element.setAttribute(attribute, String(value)));
    return element;
  }
  start();
})();
