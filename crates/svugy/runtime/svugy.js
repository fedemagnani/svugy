(() => {
  'use strict';

  const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';
  const XHTML_NAMESPACE = 'http://www.w3.org/1999/xhtml';

  const CONTRACT = 'data-svugy-contract';
  const WORKFLOW = 'data-svugy-workflow';
  const CAMERA = 'data-svugy-camera';
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
  const PANNING = 'data-svugy-panning';
  const STEPS = 'data-svugy-steps';
  const SELECTED = 'data-svugy-selected';

  const WORKFLOW_SELECTOR = `[${WORKFLOW}]`;
  const CAMERA_SELECTOR = `[${CAMERA}]`;
  const LAYER_SELECTOR = `[${LAYER}]`;
  const LEVEL_SELECTOR = `[${LEVEL}]`;
  const ELEMENT_SELECTOR = `[${ELEMENT}]`;
  const EDGE_SELECTOR = `[${EDGE}]`;
  const SHOWN_SELECTOR = `[${SHOWN}]`;
  const SHOWN_WORKFLOW_SELECTOR = `[${WORKFLOW}][${SHOWN}]`;
  const ENTERED_SELECTOR = `[${ENTERED}]`;
  const DIMMED_SELECTOR = `[${DIMMED}]`;
  const SELECTED_SELECTOR = `[${SELECTED}]`;
  const INTERACTIVE_SELECTOR = `[${ELEMENT}],[${EDGE}],[${STUB}]`;
  const HEADING_SELECTOR = `[${HEADING}]`;
  const ELEMENT_OR_LEVEL_SELECTOR = `[${ELEMENT}],[${LEVEL}]`;

  const ROOT_LEVEL_PATH = '';
  const LEVEL_LINK_PREFIX = '#svugy=';
  const LEVEL_LINK_PATH_KEY = 'svugy';
  const LEVEL_LINK_LAYER_KEY = 'layer';
  const LEVEL_LINK_WORKFLOW_KEY = 'workflow';
  const LEVEL_LINK_STEP_KEY = 'step';
  const TWEEN_DURATION_MS = 450;
  const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';
  const FADE_IN_KEYFRAMES = [{ opacity: 0 }, { opacity: 1 }];
  const FADE_OUT_KEYFRAMES = [{ opacity: 1 }, { opacity: 0 }];

  const PANEL_HIDDEN = 'hidden';
  const PANEL_OPEN = 'open';
  const PANEL_PEEK = 'peek';
  const PANEL_EXPANDED = 'expanded';
  const STEPS_CARD = 'steps';
  const COMPONENT_CARD = 'component';
  const SIDE_PANEL_MIN_BOX_WIDTH_PX = 640;
  const SIDE_PANEL_MAX_WIDTH_PX = 360;
  const SIDE_PANEL_MAX_BOX_SHARE = 0.45;
  const SIDE_PANEL_TAB_WIDTH_PX = 14;
  const SIDE_PANEL_TAB_HEIGHT_PX = 48;
  const SIDE_PANEL_MIN_WIDTH_PX = 240;
  const SIDE_PANEL_MAX_DRAGGED_BOX_SHARE = 0.7;
  const STEPS_CARD_MIN_SHARE = 0.15;
  const GRIP_KEY_STEP_PX = 24;
  const SHEET_MAX_BOX_SHARE = 0.7;
  const SHEET_HANDLE_HEIGHT_PX = 24;
  const SHEET_HANDLE_TAB_WIDTH_PX = 96;
  const SHEET_LINE_HEIGHT_PX = 40;

  const WORKFLOW_MENU_WIDTH_PX = 300;
  const WORKFLOW_MENU_HEADING_GAP_PX = 8;
  const HEADING_CARET_GAP = 6;

  const CAMERA_SWITCH_WIDTH_PX = 280;
  const CAMERA_SWITCH_HEIGHT_PX = 36;
  const CAMERA_SWITCH_VIEW_MARGIN_PX = 12;
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
  const STEP_TARGET_PATTERN = /^([^/]+)\/([^/]+)$/;
  const HEX_COLOR_PATTERN = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i;
  const RGB_COLOR_PATTERN = /^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)/i;
  const FLOW_PATH_SELECTOR = 'path[data-svugy-flow]';
  const FACE_PATH_SELECTOR = ':scope > path:not([data-svugy-hachure])';
  const LINK_HOVER_PROPERTY = '--svugy-link-hover';
  const MIN_LINK_CONTRAST = 3;

  const HIGHLIGHTED_CLASS = 'svugy-highlighted';
  const HIGHLIGHT_DURATION_MS = 1200;

  const DRAG_THRESHOLD_PX = 4;
  const ZOOM_PER_WHEEL_PIXEL = 0.002;
  const WHEEL_LINE_PX = 16;
  const MIN_VIEW_WIDTH = 80;
  const LEAF_ZOOM_CONTEXT = 3;
  const LEAF_ZOOM_MAX_LEVEL_SHARE = 0.7;
  const LEAF_ZOOM_MARGIN = 12;
  const MAX_ZOOM_OUT_CANVASES = 2;
  const VIEW_MATCH_TOLERANCE = 0.001;
  const HEADING_VIEW_MARGIN_PX = 20;
  const HEADING_SCREEN_SCALE = 1.2;

  const ARROW = '\u2192';
  const CARET = '\u25be';
  const PREVIOUS_SIGN = '\u2039';
  const NEXT_SIGN = '\u203a';
  const HIDE_SIGN = '\u203a';
  const CLEAR_SIGN = '\u2715';
  const MIDDLE_DOT = '\u00b7';

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

  /** Reads a `#svugy=PATH&layer=K&workflow=ID&step=ID` fragment of the document's URL, PATH URL-encoded, every key but `svugy` optional; null without one. */
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
    const workflowId = fragmentParameters.get(LEVEL_LINK_WORKFLOW_KEY);
    const stepId = fragmentParameters.get(LEVEL_LINK_STEP_KEY);
    return { path, layerIndex, workflowId, stepId };
  }

  /** Wires navigation, the reader's panel and scene marks, the camera switch and the workflow menu onto one diagram, then starts where a level link points, if one does. */
  function bindRuntime(svg) {
    const workflows = Workflows.of(svg);
    const renderedWorkflow = svg.querySelector(SHOWN_WORKFLOW_SELECTOR) ?? svg;
    const link = levelLink();
    const linkedWorkflow = link === null ? null : linkedWorkflowOf(link, workflows, renderedWorkflow);
    const initialWorkflow = linkedWorkflow ?? renderedWorkflow;
    const cameras = Cameras.of(initialWorkflow);
    const initialCamera = Cameras.shownOf(initialWorkflow);
    const initialLayers = Layers.of(initialCamera);
    const rootLevel = Contract.levelAt(initialLayers[0], ROOT_LEVEL_PATH);
    if (rootLevel === null) {
      return;
    }
    const uncoveredArea = new UncoveredArea(svg);
    const animator = new ViewBoxAnimator(svg, uncoveredArea);
    const fader = new LayerFader();
    const navigator = new LayerNavigator(svg, initialWorkflow, initialCamera, rootLevel, animator, fader);
    const highlighter = new TargetHighlighter();
    const reader = new Reader(svg, workflows, navigator, uncoveredArea, highlighter);
    const gestures = new ViewGestures(svg, animator, navigator);
    gestures.bind();
    reader.bind();
    const headingPins = new HeadingPins(svg);
    const hasOtherWorkflows = workflows.length > 1;
    const workflowMenu = hasOtherWorkflows ? new WorkflowMenu(svg, workflows, navigator, reader) : null;
    workflowMenu?.bind();
    headingPins.bind();
    const hasOtherCameras = cameras.length > 1;
    if (hasOtherCameras) {
      const angles = cameras.map((camera) => camera.getAttribute(CAMERA));
      const showCameraAngle = (angle) => reader.showCameraAngle(angle);
      const cameraSwitch = new CameraSwitch(svg, angles, navigator, uncoveredArea, showCameraAngle);
      cameraSwitch.bind();
    }
    Interactions.bind(svg, reader, workflowMenu);
    const isStartLinked = linkedWorkflow !== null;
    if (isStartLinked) {
      reader.startAt(link);
    }
  }

  /** Returns the workflow a level link starts in, the one it names, else `renderedWorkflow`; null when its PATH names no level of that workflow's shown camera, so the link leaves the start as it is. */
  function linkedWorkflowOf(link, workflows, renderedWorkflow) {
    const namedWorkflow = workflows.find((workflow) => {
      const workflowId = workflow.getAttribute(WORKFLOW);
      return workflowId === link.workflowId;
    });
    const linkedWorkflow = namedWorkflow ?? renderedWorkflow;
    const linkedCamera = Cameras.shownOf(linkedWorkflow);
    const linkedLayers = Layers.of(linkedCamera);
    const isLinkedLevel = linkedLayers.some((layer) => {
      const linkedLevel = Contract.levelAt(layer, link.path);
      return linkedLevel !== null;
    });
    return isLinkedLevel ? linkedWorkflow : null;
  }

  /** Lists the workflows a document draws, in written order; empty for a document without workflow groups. */
  const Workflows = {
    of(svg) {
      const workflowGroups = svg.querySelectorAll(WORKFLOW_SELECTOR);
      return Array.from(workflowGroups);
    },

    /** Reads a workflow group's details, or empty ones for a document without workflow groups. */
    detailsOf(workflow) {
      const details = Contract.detailsOf(workflow);
      const isWorkflowDetails = details !== null && Array.isArray(details.steps);
      return isWorkflowDetails ? details : { steps: [] };
    },
  };

  /** Lists the cameras a document draws the diagram from; a document without camera groups is its own single camera. */
  const Cameras = {
    of(svg) {
      const cameraGroups = Array.from(svg.querySelectorAll(CAMERA_SELECTOR));
      const hasCameraGroups = cameraGroups.length > 0;
      return hasCameraGroups ? cameraGroups : [svg];
    },

    /** Returns the camera `workflow` is rendered from, the author's, else its first. */
    shownOf(workflow) {
      const cameras = Cameras.of(workflow);
      const shownCamera = cameras.find((camera) => camera.hasAttribute(SHOWN));
      return shownCamera ?? cameras[0];
    },
  };

  /** Lists the layers a camera draws, in increasing depth; a camera without layer groups is its own single layer. */
  const Layers = {
    of(camera) {
      const layerGroups = Array.from(camera.querySelectorAll(LAYER_SELECTOR));
      const hasLayerGroups = layerGroups.length > 0;
      return hasLayerGroups ? layerGroups : [camera];
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
      return targetedEdge === undefined ? null : Contract.homeGroupOf(scope, targetedEdge);
    },

    /** Finds the group of `group`'s edge in its home level, the outermost of the edge's groups in `scope`, since a bundled edge's branches lie in its containers' levels; `group` itself when it draws no edge. */
    homeGroupOf(scope, group) {
      const edgeName = group.getAttribute(EDGE);
      if (edgeName === null) {
        return group;
      }
      const levelDepthOf = (edgeGroup) => {
        const enclosingLevel = Contract.enclosingLevelOf(edgeGroup);
        return Contract.depthOf(enclosingLevel);
      };
      const edgeGroups = Contract.edgesAt(scope, edgeName);
      return edgeGroups.reduce((homeGroup, candidate) => {
        const isCandidateOuter = levelDepthOf(candidate) < levelDepthOf(homeGroup);
        return isCandidateOuter ? candidate : homeGroup;
      });
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

    stepIdsOf(group) {
      const stepsText = group.getAttribute(STEPS) ?? '';
      const stepIds = stepsText.split(' ');
      return stepIds.filter((stepId) => stepId !== '');
    },

    /** Describes the item `group` draws, for selecting it: an element item for an element group, else an edge item. */
    itemDrawnBy(group) {
      const isElement = group.hasAttribute(ELEMENT);
      return isElement ? new ElementItem(group) : new EdgeItem(group);
    },

    /** Lists the element and edge groups of `layer` that dimming may touch: every one but badges. */
    dimmableGroupsOf(layer) {
      const layerElements = Array.from(layer.querySelectorAll(ELEMENT_SELECTOR));
      const layerEdges = Array.from(layer.querySelectorAll(EDGE_SELECTOR));
      const dimmableEdges = layerEdges.filter((edge) => !Contract.isBadge(edge));
      return [...layerElements, ...dimmableEdges];
    },

    headingOf(layer) {
      return layer.querySelector(HEADING_SELECTOR);
    },
  };

  /** Names a selected element by its path, with its title, details and steps. */
  class ElementItem {
    constructor(group) {
      this.path = group.getAttribute(ELEMENT);
      this.title = Contract.titleOf(group);
      this.details = Contract.detailsOf(group) ?? {};
      this.stepIds = Contract.stepIdsOf(group);
      this.kindLabel = 'Component';
    }

    /** Lists the groups of `layer` drawing the element: its group, else the group of its closest ancestor `layer` draws. */
    groupsIn(layer, deepestLayer) {
      const element = Contract.elementAt(layer, this.path);
      if (element !== null) {
        return [element];
      }
      const deepestElement = Contract.elementAt(deepestLayer, this.path);
      const deepestAncestors = deepestElement === null ? [] : Contract.ancestorElementsOf(deepestElement);
      const ancestorPaths = deepestAncestors.map((ancestor) => ancestor.getAttribute(ELEMENT));
      const drawnAncestors = ancestorPaths.map((path) => Contract.elementAt(layer, path));
      const closestDrawnAncestor = drawnAncestors.find((ancestor) => ancestor !== null);
      return closestDrawnAncestor === undefined ? [] : [closestDrawnAncestor];
    }

    /** Names the element: its label, else its title, else its path. */
    nameIn(_deepestLayer) {
      return this.details.label ?? this.title ?? this.path;
    }

    factNodesBy(content, _deepestLayer) {
      return content.elementNodesOf(this.details);
    }
  }

  /** Names a selected written or lifted connection by its edge name, with its title, details and steps. */
  class EdgeItem {
    constructor(group) {
      this.edgeName = group.getAttribute(EDGE);
      this.title = Contract.titleOf(group);
      this.details = Contract.detailsOf(group) ?? {};
      this.stepIds = Contract.stepIdsOf(group);
      this.isLifted = this.details.kind === LIFTED_KIND;
      this.kindLabel = 'Edge';
    }

    /** Lists the groups of `layer` drawing the connection, its label's included. */
    groupsIn(layer, _deepestLayer) {
      return Contract.edgesAt(layer, this.edgeName);
    }

    /** Names the connection: a lifted one's title, else its label, else its endpoints' labels joined by an arrow. */
    nameIn(deepestLayer) {
      if (this.isLifted) {
        return this.title ?? this.edgeName;
      }
      return this.details.label ?? CardContent.endpointsNameOf(this.details, deepestLayer);
    }

    factNodesBy(content, deepestLayer) {
      return content.connectionNodesOf(this, deepestLayer);
    }
  }

  /** Compares standpoints, the shown layer's index and the focused level's path. */
  const Standpoints = {
    areSame(standpoint, otherStandpoint) {
      const isSameLayer = standpoint.layerIndex === otherStandpoint.layerIndex;
      const isSameLevel = standpoint.levelPath === otherStandpoint.levelPath;
      return isSameLayer && isSameLevel;
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

    // width = max(w, h·A), height = width / A, A the canvas aspect; the view stays centred on what it frames, the ground reaching past every side
    fittedToAspect(view, aspect) {
      const width = Math.max(view.width, view.height * aspect);
      const height = width / aspect;
      const x = view.x + (view.width - width) / 2;
      const y = view.y + (view.height - height) / 2;
      return { x, y, width, height };
    },

    // views match when every edge lies within a thousandth of the width: fitting and rounding never move a view further
    isNear(view, other) {
      const tolerance = view.width * VIEW_MATCH_TOLERANCE;
      const isXNear = Math.abs(view.x - other.x) <= tolerance;
      const isYNear = Math.abs(view.y - other.y) <= tolerance;
      const isWidthNear = Math.abs(view.width - other.width) <= tolerance;
      const isHeightNear = Math.abs(view.height - other.height) <= tolerance;
      return isXNear && isYNear && isWidthNear && isHeightNear;
    },

    interpolate(start, target, fraction) {
      return {
        x: start.x + (target.x - start.x) * fraction,
        y: start.y + (target.y - start.y) * fraction,
        width: start.width + (target.width - start.width) * fraction,
        height: start.height + (target.height - start.height) * fraction,
      };
    },

    // per side s: max(min(k·leaf s, ρ·level s), leaf s + 2m), centred on the leaf; k = LEAF_ZOOM_CONTEXT, ρ = LEAF_ZOOM_MAX_LEVEL_SHARE, m = LEAF_ZOOM_MARGIN; width also ≥ MIN_VIEW_WIDTH
    aroundLeaf(leafBounds, levelView) {
      const leafCentre = centreOf(leafBounds);
      const contextWidth = View.leafZoomSide(leafBounds.width, levelView.width);
      const width = Math.max(contextWidth, MIN_VIEW_WIDTH);
      const height = View.leafZoomSide(leafBounds.height, levelView.height);
      return { x: leafCentre.x - width / 2, y: leafCentre.y - height / 2, width, height };
    },

    leafZoomSide(leafSide, levelSide) {
      const contextSide = LEAF_ZOOM_CONTEXT * leafSide;
      const levelShareSide = LEAF_ZOOM_MAX_LEVEL_SHARE * levelSide;
      const cappedSide = Math.min(contextSide, levelShareSide);
      const marginedLeafSide = leafSide + 2 * LEAF_ZOOM_MARGIN;
      return Math.max(cappedSide, marginedLeafSide);
    },

    // ease(t) = 1 − (1 − t)³
    easeOutCubic(fraction) {
      return 1 - (1 - fraction) ** 3;
    },
  };

  /** Tracks the uncovered area, the SVG's on-screen box minus the description panel, and fits views into it. */
  class UncoveredArea {
    constructor(svg) {
      this.svg = svg;
      this.coveredRightPx = 0;
      this.coveredBottomPx = 0;
      this.changeListeners = [];
    }

    /** Calls `listener()` whenever the covered strip changes. */
    addChangeListener(listener) {
      this.changeListeners.push(listener);
    }

    /** Records the panel covering `rightPx` of the box's right edge and `bottomPx` of its bottom edge. */
    cover(rightPx, bottomPx) {
      const isSameCover = rightPx === this.coveredRightPx && bottomPx === this.coveredBottomPx;
      if (isSameCover) {
        return;
      }
      this.coveredRightPx = rightPx;
      this.coveredBottomPx = bottomPx;
      this.changeListeners.forEach((listener) => listener());
    }

    clientRect() {
      const box = this.svg.getBoundingClientRect();
      const width = Math.max(0, box.width - this.coveredRightPx);
      const height = Math.max(0, box.height - this.coveredBottomPx);
      return { left: box.left, top: box.top, width, height, right: box.left + width, bottom: box.top + height };
    }

    // s = min(U_w / v_w, U_h / v_h) px per unit; W = min(B_w, B_h·A) / s, H = W / A; o = (B − s·(W, H)) / 2 the meet letterbox; x = c_x − (U_w/2 − o_x) / s, likewise y: v's centre c lands on U's centre
    viewBoxShowing(view, canvasAspect) {
      const box = this.svg.getBoundingClientRect();
      const uncovered = this.clientRect();
      const isUnmeasurable = uncovered.width === 0 || uncovered.height === 0;
      if (isUnmeasurable) {
        return View.fittedToAspect(view, canvasAspect);
      }
      const pixelsPerUnit = Math.min(uncovered.width / view.width, uncovered.height / view.height);
      const boxWidthAtAspect = Math.min(box.width, box.height * canvasAspect);
      const width = boxWidthAtAspect / pixelsPerUnit;
      const height = width / canvasAspect;
      const letterboxX = (box.width - pixelsPerUnit * width) / 2;
      const letterboxY = (box.height - pixelsPerUnit * height) / 2;
      const viewCentre = centreOf(view);
      const x = viewCentre.x - (uncovered.width / 2 - letterboxX) / pixelsPerUnit;
      const y = viewCentre.y - (uncovered.height / 2 - letterboxY) / pixelsPerUnit;
      return { x, y, width, height };
    }
  }

  /** Tweens or jumps the root viewBox to a target view fitted into the uncovered area at the canvas aspect, so changing view never changes the document's shape; tweens jump under reduced motion. */
  class ViewBoxAnimator {
    constructor(svg, uncoveredArea) {
      this.svg = svg;
      this.uncoveredArea = uncoveredArea;
      this.frameRequest = 0;
      this.canvasView = this.currentView();
      this.canvasAspect = this.canvasView.width / this.canvasView.height;
    }

    tweenTo(requestedView, startView = this.currentView()) {
      const isMotionReduced = window.matchMedia(REDUCED_MOTION_QUERY).matches;
      if (isMotionReduced) {
        this.jumpTo(requestedView);
        return;
      }
      cancelAnimationFrame(this.frameRequest);
      const targetView = this.fitted(requestedView);
      this.show(startView);
      let startTime = null;
      const step = (now) => {
        startTime ??= now;
        const elapsedFraction = Math.min(1, (now - startTime) / TWEEN_DURATION_MS);
        const easedFraction = View.easeOutCubic(elapsedFraction);
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
      const targetView = this.fitted(requestedView);
      this.jumpToViewBox(targetView);
    }

    /** Shows `viewBox` as it is, ending any tween: for gestures, whose views are already viewBoxes. */
    jumpToViewBox(viewBox) {
      cancelAnimationFrame(this.frameRequest);
      this.show(viewBox);
    }

    /** Returns the viewBox showing `view` fitted into the uncovered area and centred in it. */
    fitted(view) {
      return this.uncoveredArea.viewBoxShowing(view, this.canvasAspect);
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

  /** Lets the reader look around the shown view: dragging pans it, pinching or Ctrl/⌘ + scrolling zooms it around the pointer; a one-finger touch keeps scrolling the page. */
  class ViewGestures {
    constructor(svg, animator, navigator) {
      this.svg = svg;
      this.animator = animator;
      this.navigator = navigator;
      this.pointerPoints = new Map();
      this.dragOrigin = null;
      this.isDragging = false;
      this.isClickSwallowed = false;
      this.safariPinchScale = 1;
    }

    bind() {
      this.svg.addEventListener('pointerdown', (event) => this.startPointer(event));
      this.svg.addEventListener('pointermove', (event) => this.movePointer(event));
      this.svg.addEventListener('pointerup', (event) => this.endPointer(event));
      this.svg.addEventListener('pointercancel', (event) => this.endPointer(event));
      // capture, so the click a drag ends with never reaches navigation
      this.svg.addEventListener('click', (event) => this.swallowClickOfDrag(event), true);
      this.svg.addEventListener('wheel', (event) => this.zoomOnModifiedWheel(event), { passive: false });
      // Safari reports trackpad pinches as gesture events, not as Ctrl + wheel
      this.svg.addEventListener('gesturestart', (event) => this.startSafariPinch(event));
      this.svg.addEventListener('gesturechange', (event) => this.zoomOnSafariPinch(event));
    }

    startPointer(event) {
      const isOtherMouseButton = event.pointerType === 'mouse' && event.button !== 0;
      if (isOtherMouseButton) return;
      const clientPoint = { x: event.clientX, y: event.clientY };
      this.pointerPoints.set(event.pointerId, clientPoint);
      this.dragOrigin = clientPoint;
    }

    movePointer(event) {
      const previousPoint = this.pointerPoints.get(event.pointerId);
      if (previousPoint === undefined) return;
      const clientPoint = { x: event.clientX, y: event.clientY };
      const isPinch = this.pointerPoints.size === 2;
      if (isPinch) {
        this.pinch(event.pointerId, clientPoint);
        return;
      }
      const isOneFingerTouch = event.pointerType === 'touch';
      if (isOneFingerTouch) return;
      const isDragStarting = !this.isDragging;
      const dragDistance = distanceBetween(clientPoint, this.dragOrigin);
      const isStillAClick = isDragStarting && dragDistance < DRAG_THRESHOLD_PX;
      if (isStillAClick) return;
      if (isDragStarting) this.startDrag(event.pointerId);
      this.pointerPoints.set(event.pointerId, clientPoint);
      const pannedX = clientPoint.x - previousPoint.x;
      const pannedY = clientPoint.y - previousPoint.y;
      this.panBy(pannedX, pannedY);
    }

    // capturing only once the pointer moved keeps a plain click aimed at the group under it
    startDrag(pointerId) {
      this.isDragging = true;
      this.svg.setPointerCapture(pointerId);
      this.svg.setAttribute(PANNING, '');
    }

    pinch(pointerId, clientPoint) {
      const [firstBefore, secondBefore] = this.pointerPoints.values();
      const midpointBefore = midpointOf(firstBefore, secondBefore);
      const spanBefore = distanceBetween(firstBefore, secondBefore);
      this.pointerPoints.set(pointerId, clientPoint);
      const [firstAfter, secondAfter] = this.pointerPoints.values();
      const midpointAfter = midpointOf(firstAfter, secondAfter);
      const spanAfter = distanceBetween(firstAfter, secondAfter);
      this.isDragging = true;
      if (spanAfter === 0) return;
      const zoomFactor = spanBefore / spanAfter;
      this.zoomAround(midpointAfter, zoomFactor);
      const pannedX = midpointAfter.x - midpointBefore.x;
      const pannedY = midpointAfter.y - midpointBefore.y;
      this.panBy(pannedX, pannedY);
    }

    endPointer(event) {
      this.pointerPoints.delete(event.pointerId);
      const isGestureOngoing = this.pointerPoints.size > 0;
      if (isGestureOngoing) return;
      const wasDragging = this.isDragging;
      this.isDragging = false;
      this.svg.removeAttribute(PANNING);
      if (wasDragging) {
        // the click a drag ends with follows pointerup in the same task
        this.isClickSwallowed = true;
        setTimeout(() => { this.isClickSwallowed = false; }, 0);
      }
    }

    swallowClickOfDrag(event) {
      const isClickOfDrag = this.isClickSwallowed;
      if (isClickOfDrag) {
        event.stopImmediatePropagation();
        this.isClickSwallowed = false;
      }
    }

    // factor = e^(k · Δy): equal scroll distances zoom by equal ratios
    zoomOnModifiedWheel(event) {
      const isPageScroll = !event.ctrlKey && !event.metaKey;
      if (isPageScroll) return;
      event.preventDefault();
      const pixelsPerDeltaUnit = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? WHEEL_LINE_PX : 1;
      const wheelPixels = event.deltaY * pixelsPerDeltaUnit;
      const zoomFactor = Math.exp(wheelPixels * ZOOM_PER_WHEEL_PIXEL);
      const clientPoint = { x: event.clientX, y: event.clientY };
      this.zoomAround(clientPoint, zoomFactor);
    }

    startSafariPinch(event) {
      event.preventDefault();
      this.safariPinchScale = 1;
    }

    zoomOnSafariPinch(event) {
      event.preventDefault();
      const zoomFactor = this.safariPinchScale / event.scale;
      this.safariPinchScale = event.scale;
      const clientPoint = { x: event.clientX, y: event.clientY };
      this.zoomAround(clientPoint, zoomFactor);
    }

    panBy(pannedX, pannedY) {
      const view = this.animator.currentView();
      const screenMatrix = this.svg.getScreenCTM();
      const userUnitsPerPixel = 1 / screenMatrix.a;
      const pannedView = {
        ...view,
        x: view.x - pannedX * userUnitsPerPixel,
        y: view.y - pannedY * userUnitsPerPixel,
      };
      this.animator.jumpToViewBox(pannedView);
    }

    /** Zooms in around the pointer, and out toward the focused level's view, so zooming back out lands on it; the width stays within [MIN_VIEW_WIDTH, MAX_ZOOM_OUT_CANVASES · canvas width]. */
    zoomAround(clientPoint, zoomFactor) {
      const view = this.animator.currentView();
      const canvasWidth = this.animator.canvasView.width;
      const widestView = MAX_ZOOM_OUT_CANVASES * canvasWidth;
      const requestedWidth = view.width * zoomFactor;
      const atLeastMinWidth = Math.max(MIN_VIEW_WIDTH, requestedWidth);
      const zoomedWidth = Math.min(widestView, atLeastMinWidth);
      const appliedFactor = zoomedWidth / view.width;
      const isZoomingOut = appliedFactor > 1;
      const zoomedView = isZoomingOut
        ? this.zoomedOutTowardHome(view, appliedFactor)
        : this.zoomedInAround(clientPoint, view, appliedFactor);
      this.animator.jumpToViewBox(zoomedView);
    }

    // the point under the pointer stays put: x' = p − (p − x) · f
    zoomedInAround(clientPoint, view, appliedFactor) {
      const userPoint = this.userPointAt(clientPoint);
      return {
        x: userPoint.x - (userPoint.x - view.x) * appliedFactor,
        y: userPoint.y - (userPoint.y - view.y) * appliedFactor,
        width: view.width * appliedFactor,
        height: view.height * appliedFactor,
      };
    }

    // c' = c + (h − c) · min(1, (w' − w) / (H − w)), c the view's centre, h and H the home view's centre and width: the centre reaches home as the width does
    zoomedOutTowardHome(view, appliedFactor) {
      const focusedView = this.navigator.focusedView();
      const homeView = this.animator.fitted(focusedView);
      const zoomedWidth = view.width * appliedFactor;
      const zoomedHeight = view.height * appliedFactor;
      const widthLeftToHome = homeView.width - view.width;
      const widthGained = zoomedWidth - view.width;
      const isAtOrPastHome = widthLeftToHome <= 0;
      const homeward = isAtOrPastHome ? 1 : Math.min(1, widthGained / widthLeftToHome);
      const centre = centreOf(view);
      const homeCentre = centreOf(homeView);
      const zoomedCentreX = centre.x + (homeCentre.x - centre.x) * homeward;
      const zoomedCentreY = centre.y + (homeCentre.y - centre.y) * homeward;
      return {
        x: zoomedCentreX - zoomedWidth / 2,
        y: zoomedCentreY - zoomedHeight / 2,
        width: zoomedWidth,
        height: zoomedHeight,
      };
    }

    userPointAt(clientPoint) {
      const screenPoint = new DOMPoint(clientPoint.x, clientPoint.y);
      const screenMatrix = this.svg.getScreenCTM();
      const userMatrix = screenMatrix.inverse();
      return screenPoint.matrixTransform(userMatrix);
    }
  }

  /** Pins every layer's heading inside the view's top-left corner at a constant on-screen size, wherever the view moves; without script it stays where rendered. */
  class HeadingPins {
    constructor(svg) {
      this.svg = svg;
      this.headings = Array.from(svg.querySelectorAll(HEADING_SELECTOR));
    }

    bind() {
      const viewBoxObserver = new MutationObserver(() => this.pin());
      viewBoxObserver.observe(this.svg, { attributes: true, attributeFilter: ['viewBox'] });
      const resizeObserver = new ResizeObserver(() => this.pin());
      resizeObserver.observe(this.svg);
      this.pin();
    }

    // s = k·u and t = corner + m·u − s·b, with u user units per pixel and b the heading's own top-left, so it sits m pixels inside the top-left corner at k times its size
    pin() {
      const screenMatrix = this.svg.getScreenCTM();
      if (screenMatrix === null) return;
      const screenToUserMatrix = screenMatrix.inverse();
      const svgBounds = this.svg.getBoundingClientRect();
      const clientCorner = new DOMPoint(svgBounds.left, svgBounds.top);
      const userCorner = clientCorner.matrixTransform(screenToUserMatrix);
      const userUnitsPerPixel = 1 / Math.hypot(screenMatrix.a, screenMatrix.b);
      const scale = HEADING_SCREEN_SCALE * userUnitsPerPixel;
      const margin = HEADING_VIEW_MARGIN_PX * userUnitsPerPixel;
      this.headings.forEach((heading) => {
        const headingBounds = heading.getBBox();
        const x = userCorner.x + margin - scale * headingBounds.x;
        const y = userCorner.y + margin - scale * headingBounds.y;
        heading.setAttribute('transform', `translate(${x} ${y}) scale(${scale})`);
      });
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
      const timing = { duration: TWEEN_DURATION_MS, easing: 'ease-out' };
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

  /** Keeps one workflow, one of its cameras and one of its layers shown and one of its levels focused, and moves the view to them. */
  class LayerNavigator {
    constructor(svg, initialWorkflow, initialCamera, rootLevel, animator, fader) {
      this.svg = svg;
      this.layers = Layers.of(initialCamera);
      this.animator = animator;
      this.fader = fader;
      const staleMarks = svg.querySelectorAll(`${SHOWN_SELECTOR},${ENTERED_SELECTOR}`);
      staleMarks.forEach((node) => {
        node.removeAttribute(SHOWN);
        node.removeAttribute(ENTERED);
      });
      const initialLayer = this.layers[0];
      initialWorkflow.setAttribute(SHOWN, '');
      initialCamera.setAttribute(SHOWN, '');
      initialLayer.setAttribute(SHOWN, '');
      rootLevel.setAttribute(ENTERED, '');
      this.shownWorkflow = initialWorkflow;
      this.shownCamera = initialCamera;
      this.shownLayer = initialLayer;
      this.focusedLevel = rootLevel;
      this.navigationListeners = [];
    }

    /** Calls `listener()` after every change of the shown workflow, camera or layer or the focused level. */
    addNavigationListener(listener) {
      this.navigationListeners.push(listener);
    }

    /** Shows `workflow` from the shown camera's angle, at the root of its layer 0, jumping to the root's view; tells whether the workflow changed. */
    showWorkflow(workflow) {
      const isShownWorkflow = workflow === this.shownWorkflow;
      if (isShownWorkflow) {
        return false;
      }
      const shownAngle = this.shownCamera.getAttribute(CAMERA);
      const cameras = Cameras.of(workflow);
      const sameAngleCamera = cameras.find((camera) => camera.getAttribute(CAMERA) === shownAngle);
      const camera = sameAngleCamera ?? cameras[0];
      const layers = Layers.of(camera);
      const firstLayer = layers[0];
      const rootLevel = Contract.levelAt(firstLayer, ROOT_LEVEL_PATH);
      if (rootLevel === null) {
        return false;
      }
      this.fader.finishRunningFade();
      const previousLayer = this.shownLayer;
      const previousMarks = [this.shownWorkflow, this.shownCamera, this.shownLayer];
      previousMarks.forEach((node) => node.removeAttribute(SHOWN));
      const shownMarks = [workflow, camera, firstLayer];
      shownMarks.forEach((node) => node.setAttribute(SHOWN, ''));
      this.shownWorkflow = workflow;
      this.shownCamera = camera;
      this.layers = layers;
      this.shownLayer = firstLayer;
      this.moveEnteredMarkTo(rootLevel);
      this.announceNavigation();
      this.carryKeyboardFocus(previousLayer, firstLayer);
      const rootView = this.viewOf(rootLevel);
      if (rootView !== null) {
        this.animator.jumpTo(rootView);
      }
      return true;
    }

    /** Discloses a step: shows layer max(K, `layerIndex`), focused on the root when a group of it taking part in `stepId` lies outside the focused level, neither inside it nor enclosing it; tells whether the view moved. */
    discloseStep(layerIndex, stepId) {
      const shownLayerIndex = this.layers.indexOf(this.shownLayer);
      const disclosingLayerIndex = Math.max(shownLayerIndex, layerIndex);
      const disclosingLayer = this.layers[disclosingLayerIndex] ?? this.shownLayer;
      const focusedPath = this.focusedLevel.getAttribute(LEVEL);
      const focusedLevelThere = Contract.levelAt(disclosingLayer, focusedPath);
      const disclosingGroups = Contract.dimmableGroupsOf(disclosingLayer);
      const stepGroups = disclosingGroups.filter((group) => {
        const groupStepIds = Contract.stepIdsOf(group);
        return groupStepIds.includes(stepId);
      });
      const isStepOutsideFocusedLevel = stepGroups.some((group) => {
        const isInsideFocusedLevel = focusedLevelThere.contains(group);
        const isEnclosingFocusedLevel = group.contains(focusedLevelThere);
        return !isInsideFocusedLevel && !isEnclosingFocusedLevel;
      });
      const disclosedPath = isStepOutsideFocusedLevel ? ROOT_LEVEL_PATH : focusedPath;
      return this.focusLevelIn(disclosingLayer, disclosedPath, this.focusedLevel);
    }

    /** Returns the layer drawing the shown workflow whole, from the shown camera. */
    deepestLayer() {
      return this.layers.at(-1);
    }

    /** Shows layer `layerIndex` focused on level `path`, without animation, or the first layer holding that level when layer `layerIndex` does not; a path that is no level changes nothing. */
    jumpTo(path, layerIndex) {
      const requestedLayer = this.layers[layerIndex];
      const levelInRequestedLayer = requestedLayer === undefined ? null : Contract.levelAt(requestedLayer, path);
      const isLevelInRequestedLayer = levelInRequestedLayer !== null;
      const firstOpeningLayer = this.layers.find((layer) => {
        const level = Contract.levelAt(layer, path);
        return level !== null;
      });
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

    /** Shows the shown workflow from its camera at `angle`, on its layer as deep as the shown one, focused on the same level, framed by that level's own view; tells whether the camera changed. */
    showCameraAngle(angle) {
      const cameras = Cameras.of(this.shownWorkflow);
      const camera = cameras.find((candidate) => candidate.getAttribute(CAMERA) === angle);
      const isShownOrMissingCamera = camera === undefined || camera === this.shownCamera;
      if (isShownOrMissingCamera) {
        return false;
      }
      const cameraLayers = Layers.of(camera);
      const shownLayerIndex = this.layers.indexOf(this.shownLayer);
      const layer = cameraLayers[shownLayerIndex];
      const focusedPath = this.focusedLevel.getAttribute(LEVEL);
      const level = layer === undefined ? null : Contract.levelAt(layer, focusedPath);
      if (level === null) {
        return false;
      }
      cameraLayers.forEach((cameraLayer) => cameraLayer.removeAttribute(SHOWN));
      layer.setAttribute(SHOWN, '');
      const previousCamera = this.shownCamera;
      const previousLayer = this.shownLayer;
      this.shownCamera = camera;
      this.layers = cameraLayers;
      this.shownLayer = layer;
      this.moveEnteredMarkTo(level);
      // swapped, not crossfaded: the cameras project to different coordinates, so the fading one would show misframed
      this.fader.finishRunningFade();
      previousCamera.removeAttribute(SHOWN);
      camera.setAttribute(SHOWN, '');
      this.announceNavigation();
      this.carryKeyboardFocus(previousLayer, layer);
      const targetView = this.viewOf(level);
      if (targetView !== null) {
        this.animator.jumpTo(targetView);
      }
      return true;
    }

    /** Follows a click on `group`; tells whether the view moved. */
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
      const isElement = group.hasAttribute(ELEMENT);
      const interiorLevel = Contract.interiorLevelOf(group);
      const isLeaf = isElement && interiorLevel === null;
      if (isLeaf) {
        return this.zoomOntoLeaf(group);
      }
      const homeGroup = Contract.homeGroupOf(this.shownLayer, group);
      const reachedLevel = this.levelReachedThrough(homeGroup);
      return this.focus(reachedLevel);
    }

    /** Focuses `leaf`'s enclosing level and zooms onto the leaf, its neighbours still in sight, as a zoom away from that level's view; tells whether the view moved. */
    zoomOntoLeaf(leaf) {
      const enclosingLevel = Contract.enclosingLevelOf(leaf);
      this.focus(enclosingLevel);
      const leafBounds = this.userBoundsOf(leaf);
      const levelView = this.viewOf(enclosingLevel);
      const isUnmeasurable = leafBounds === null || levelView === null;
      if (isUnmeasurable) {
        return false;
      }
      const leafView = View.aroundLeaf(leafBounds, levelView);
      const currentView = this.animator.currentView();
      const fittedLeafView = this.animator.fitted(leafView);
      const isAwayFromLeafView = !View.isNear(currentView, fittedLeafView);
      if (isAwayFromLeafView) {
        this.animator.tweenTo(leafView);
      }
      return isAwayFromLeafView;
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

    /** Tells where the reader stands: the shown layer's index and the focused level's path, the same in every camera. */
    standpoint() {
      const layerIndex = this.layers.indexOf(this.shownLayer);
      const levelPath = this.focusedLevel.getAttribute(LEVEL);
      return { layerIndex, levelPath };
    }

    /** Returns to `standpoint` at its level's own view, discarding any zoom or pan. */
    returnTo(standpoint) {
      const layer = this.layers[standpoint.layerIndex];
      const hasMoved = this.focusLevelIn(layer, standpoint.levelPath, this.focusedLevel);
      if (hasMoved) {
        return;
      }
      this.returnToFocusedView();
    }

    /** Animates back to the focused level's own view when the reader zoomed or panned away from it; returns whether it did. */
    returnToFocusedView() {
      const currentView = this.animator.currentView();
      const focusedView = this.focusedView();
      const fittedFocusedView = this.animator.fitted(focusedView);
      const isAwayFromFocusedView = !View.isNear(currentView, fittedFocusedView);
      if (isAwayFromFocusedView) {
        this.animator.tweenTo(focusedView);
      }
      return isAwayFromFocusedView;
    }

    /** Steps out once: focuses the focused level's parent, keeping the open containers open, so every step widens the view; at the root, closes the deepest open containers; any navigation discards a pan or zoom. Tells whether it stepped out: never from the root of layer 0. */
    exitOneStep() {
      const parentLevel = Contract.enclosingLevelOf(this.focusedLevel);
      if (parentLevel !== null) {
        this.focus(parentLevel);
        return true;
      }
      const shownLayerIndex = this.layers.indexOf(this.shownLayer);
      const isOnFirstLayer = shownLayerIndex === 0;
      if (isOnFirstLayer) {
        return false;
      }
      const shallowerLayer = this.layers[shownLayerIndex - 1];
      const focusedPath = this.focusedLevel.getAttribute(LEVEL);
      this.showLayer(shallowerLayer, focusedPath, this.focusedLevel);
      return true;
    }

    /** Returns the focused level's own view, as its `data-svugy-view` frames it. */
    focusedView() {
      return this.viewOf(this.focusedLevel);
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
      const userBounds = this.userBoundsOf(node);
      return userBounds === null ? null : centreOf(userBounds);
    }

    /** Returns `node`'s on-screen box in user units, wherever the view is, or null before the SVG is laid out. */
    userBoundsOf(node) {
      const screenMatrix = this.svg.getScreenCTM();
      if (screenMatrix === null) {
        return null;
      }
      const screenToUserMatrix = screenMatrix.inverse();
      const clientBounds = node.getBoundingClientRect();
      const clientTopLeft = new DOMPoint(clientBounds.left, clientBounds.top);
      const clientBottomRight = new DOMPoint(clientBounds.right, clientBounds.bottom);
      const userTopLeft = clientTopLeft.matrixTransform(screenToUserMatrix);
      const userBottomRight = clientBottomRight.matrixTransform(screenToUserMatrix);
      return {
        x: userTopLeft.x,
        y: userTopLeft.y,
        width: userBottomRight.x - userTopLeft.x,
        height: userBottomRight.y - userTopLeft.y,
      };
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

  /** Lets the reader switch the angle the diagram is seen from: one button per camera angle, pinned inside the uncovered area's bottom-left corner at a constant on-screen size. */
  class CameraSwitch {
    constructor(svg, angles, navigator, uncoveredArea, showCameraAngle) {
      this.svg = svg;
      this.navigator = navigator;
      this.uncoveredArea = uncoveredArea;
      this.showCameraAngle = showCameraAngle;
      this.body = createXhtmlElement('div', { class: 'svugy-cameras', role: 'group', 'aria-label': 'view angle' });
      const buttonEntries = angles.map((angle) => {
        const button = this.buttonShowing(angle);
        return [angle, button];
      });
      this.buttonsByAngle = new Map(buttonEntries);
      this.frame = createSvgElement('foreignObject', {
        class: 'svugy-cameras-frame',
        width: CAMERA_SWITCH_WIDTH_PX,
        height: CAMERA_SWITCH_HEIGHT_PX,
      });
      this.frame.appendChild(this.body);
      this.panel = createSvgElement('g', { class: 'svugy-cameras-panel' });
      this.panel.appendChild(this.frame);
      isolateOverlayEvents(this.panel);
    }

    bind() {
      this.svg.appendChild(this.panel);
      const viewBoxObserver = new MutationObserver(() => this.place());
      viewBoxObserver.observe(this.svg, { attributes: true, attributeFilter: ['viewBox'] });
      const resizeObserver = new ResizeObserver(() => this.place());
      resizeObserver.observe(this.svg);
      this.uncoveredArea.addChangeListener(() => this.place());
      this.navigator.addNavigationListener(() => this.pressShownCameraButton());
      this.pressShownCameraButton();
      this.place();
    }

    buttonShowing(angle) {
      const button = createXhtmlElement('button', { type: 'button' });
      button.textContent = angle;
      button.addEventListener('click', () => this.showCameraAngle(angle));
      this.body.appendChild(button);
      return button;
    }

    pressShownCameraButton() {
      const shownAngle = this.navigator.shownCamera.getAttribute(CAMERA);
      this.buttonsByAngle.forEach((button, angle) => {
        const isShownAngle = angle === shownAngle;
        button.setAttribute('aria-pressed', String(isShownAngle));
      });
    }

    // top-left = (U_left + m, U_bottom − m − h): m pixels inside the uncovered area's bottom-left corner
    place() {
      const uncovered = this.uncoveredArea.clientRect();
      const clientCorner = {
        x: uncovered.left + CAMERA_SWITCH_VIEW_MARGIN_PX,
        y: uncovered.bottom - CAMERA_SWITCH_VIEW_MARGIN_PX - CAMERA_SWITCH_HEIGHT_PX,
      };
      pinOverlayAt(this.svg, this.panel, clientCorner);
    }
  }

  /** Pins `overlay`'s top-left at a client point, one overlay unit per screen pixel, wherever the view is. */
  function pinOverlayAt(svg, overlay, clientPoint) {
    const screenMatrix = svg.getScreenCTM();
    if (screenMatrix === null) {
      return;
    }
    const screenToUserMatrix = screenMatrix.inverse();
    const clientDomPoint = new DOMPoint(clientPoint.x, clientPoint.y);
    const userPoint = clientDomPoint.matrixTransform(screenToUserMatrix);
    const userUnitsPerPixel = 1 / Math.hypot(screenMatrix.a, screenMatrix.b);
    overlay.setAttribute('transform', `translate(${userPoint.x} ${userPoint.y}) scale(${userUnitsPerPixel})`);
  }

  /** Keeps an overlay's pointer, click and wheel events from reaching the diagram, so they never pan, zoom or navigate. */
  function isolateOverlayEvents(overlay) {
    const isolatedEventNames = ['pointerdown', 'click', 'wheel'];
    isolatedEventNames.forEach((eventName) => {
      overlay.addEventListener(eventName, (event) => event.stopPropagation());
    });
  }

  /** Orchestrates the reader's intents: applies each to the reader state and lays the panel out for it, then navigates, so every fit fills the area the new panel leaves uncovered, and paints the scene marks last. */
  class Reader {
    constructor(svg, workflows, navigator, uncoveredArea, highlighter) {
      this.workflows = workflows;
      this.navigator = navigator;
      this.highlighter = highlighter;
      const workflowDetails = Workflows.detailsOf(navigator.shownWorkflow);
      const svgBox = svg.getBoundingClientRect();
      this.state = new ReaderState(workflowDetails, svgBox.width);
      this.selectionTrail = null;
      this.marks = new SceneMarks(svg, this.state);
      const linkTargets = new LinkTargets(workflows, navigator);
      this.panel = new DescriptionPanel(svg, uncoveredArea, this, linkTargets);
    }

    /** Attaches the panel to the diagram and draws the start state. */
    bind() {
      this.panel.bind();
      this.renderPanel();
      this.paintMarks();
    }

    /** Starts where a level link points: on its step, when it names one of the shown workflow, then at its level. */
    startAt(link) {
      this.applyThenNavigate((state) => state.goToStep(link.stepId), (startingStep) => {
        this.navigator.jumpTo(link.path, link.layerIndex);
        this.disclose(startingStep);
      });
    }

    /** Applies `transition(state)`, then discloses the step that starts dimming, if one does. */
    apply(transition) {
      this.applyThenNavigate(transition, (startingStep) => this.disclose(startingStep));
    }

    /** Applies `transition(state)` and lays the panel out for it, then calls `navigate(startingStep)`, `startingStep` being the step that starts dimming or null, then paints the scene marks. */
    applyThenNavigate(transition, navigate) {
      const dimmingStepBefore = this.state.dimmingStep();
      transition(this.state);
      this.renderPanel();
      const dimmingStep = this.state.dimmingStep();
      const isStepStartingToDim = dimmingStep !== null && dimmingStep !== dimmingStepBefore;
      const startingStep = isStepStartingToDim ? dimmingStep : null;
      navigate(startingStep);
      this.paintMarks();
    }

    disclose(step) {
      const isNoStep = step === null;
      if (isNoStep) {
        return;
      }
      this.navigator.discloseStep(step.layer, step.id);
    }

    /** Follows a click on an element, edge, badge or stub; one that jumps nowhere else first selects the clicked item, its navigation winning over the step the opening panel starts dimming. */
    activateGroup(group) {
      const followClick = () => this.navigator.followClickOn(group);
      const isJumpOrStub = group.hasAttribute(GOTO);
      if (isJumpOrStub) {
        followClick();
        this.paintMarks();
        return;
      }
      const item = Contract.itemDrawnBy(group);
      this.selectThenNavigate(item, followClick);
    }

    /** Selects `item`, then navigates, remembering the standpoint before the first of consecutive selections and the one this selection reaches. */
    selectThenNavigate(item, navigate) {
      const departureCandidate = this.navigator.standpoint();
      const previousTrail = this.selectionTrail;
      const hasSelection = this.state.selection !== null;
      const isStillWhereSelectionLeftIt = previousTrail !== null && Standpoints.areSame(previousTrail.reached, departureCandidate);
      const isChainedSelection = hasSelection && isStillWhereSelectionLeftIt;
      const departure = isChainedSelection ? previousTrail.departure : departureCandidate;
      this.applyThenNavigate((state) => state.select(item), navigate);
      const reached = this.navigator.standpoint();
      this.selectionTrail = { departure, reached };
    }

    /** Returns to where the reader stood before selecting, when it has not navigated since the selection; else to the focused level's view. */
    returnFrom(selectionTrail) {
      const currentStandpoint = this.navigator.standpoint();
      const isTrailCurrent = selectionTrail !== null && Standpoints.areSame(selectionTrail.reached, currentStandpoint);
      if (isTrailCurrent) {
        this.navigator.returnTo(selectionTrail.departure);
        return;
      }
      this.navigator.returnToFocusedView();
    }

    /** Follows a description link: selects, reveals and highlights an element or edge, or goes to a step, switching workflow first when the step is another's. */
    followLink(target) {
      const isStepTarget = target.group === undefined;
      if (isStepTarget) {
        this.goToStepOf(target.workflowId, target.stepId);
        return;
      }
      const item = Contract.itemDrawnBy(target.group);
      const revealAndHighlight = () => {
        this.navigator.revealGroup(target.group);
        const shownLayer = this.navigator.shownLayer;
        const deepestLayer = this.navigator.deepestLayer();
        const shownGroups = item.groupsIn(shownLayer, deepestLayer);
        this.highlighter.highlight(shownGroups);
      };
      this.selectThenNavigate(item, revealAndHighlight);
    }

    goToStepOf(workflowId, stepId) {
      const workflow = this.workflows.find((candidate) => {
        const candidateId = candidate.getAttribute(WORKFLOW);
        return candidateId === workflowId;
      });
      if (workflow === undefined) {
        return;
      }
      const workflowDetails = Workflows.detailsOf(workflow);
      this.applyThenNavigate((state) => state.goToStepOf(workflowDetails, stepId), (startingStep) => {
        this.navigator.showWorkflow(workflow);
        this.disclose(startingStep);
      });
    }

    /** Shows `workflow` from the root of its layer 0, the panel hidden. */
    switchWorkflow(workflow) {
      const workflowDetails = Workflows.detailsOf(workflow);
      this.applyThenNavigate((state) => state.showWorkflow(workflowDetails), () => this.navigator.showWorkflow(workflow));
    }

    showCameraAngle(angle) {
      this.navigator.showCameraAngle(angle);
      this.paintMarks();
    }

    /** Undoes one step: clears the selected item and returns from it, else returns to the focused level's view, else steps out, else returns to the workflow's start. */
    escape() {
      const hasSelection = this.state.selection !== null;
      if (hasSelection) {
        this.deselect();
        return;
      }
      const hasReturnedToFocusedView = this.navigator.returnToFocusedView();
      if (hasReturnedToFocusedView) {
        return;
      }
      const hasSteppedOut = this.navigator.exitOneStep();
      if (hasSteppedOut) {
        this.paintMarks();
        return;
      }
      this.apply((state) => state.returnToStart());
    }

    /** Clears the selected item, collapsing the component card, and returns from it: its highlight and its focus go together. */
    deselect() {
      const selectionTrail = this.selectionTrail;
      this.selectionTrail = null;
      this.applyThenNavigate((state) => state.clearSelection(), () => this.returnFrom(selectionTrail));
    }

    /** Collapses the component card while its body shows, deselecting its item; else expands it. */
    toggleComponentCard() {
      const isComponentBodyShowing = this.state.isComponentBodyShowing();
      if (isComponentBodyShowing) {
        this.deselect();
        return;
      }
      const itemOfStepItem = (stepItem) => this.itemOfStepItem(stepItem);
      this.apply((state) => state.expandComponentCard(itemOfStepItem));
    }

    /** Describes the component a step item names, as layer D draws it: its element, or its edge's source; null when layer D draws neither. */
    itemOfStepItem(stepItem) {
      const deepestLayer = this.navigator.deepestLayer();
      const isEdgeItem = stepItem.edge !== undefined;
      const edgeGroups = isEdgeItem ? Contract.edgesAt(deepestLayer, stepItem.edge) : [];
      const edgeSourcePath = edgeGroups.length === 0 ? null : edgeGroups[0].getAttribute(FROM);
      const elementPath = stepItem.element ?? edgeSourcePath;
      const element = elementPath === null ? null : Contract.elementAt(deepestLayer, elementPath);
      return element === null ? null : Contract.itemDrawnBy(element);
    }

    hover(group) {
      const isHoverChanged = this.marks.hover(group);
      if (isHoverChanged) {
        this.paintMarks();
      }
    }

    renderPanel() {
      const deepestLayer = this.navigator.deepestLayer();
      this.panel.render(deepestLayer);
    }

    paintMarks() {
      const shownLayer = this.navigator.shownLayer;
      const deepestLayer = this.navigator.deepestLayer();
      this.marks.paint(shownLayer, deepestLayer);
    }
  }

  /** Decides the reader's workflow, slot, panel and selected item, and which groups they dim; reads and writes no DOM. */
  class ReaderState {
    constructor(workflow, boxWidth) {
      const panelKind = panelKindFor(boxWidth);
      this.panel = panelKind.hidden();
      this.restartIn(workflow);
    }

    /** Shows `workflow` with the panel hidden, nothing selected and its first slot. */
    showWorkflow(workflow) {
      this.restartIn(workflow);
      this.panel = this.panel.hidden();
    }

    /** Goes to step `stepId` of `workflow`, keeping the panel and restarting in `workflow` first when it is not the shown one. */
    goToStepOf(workflow, stepId) {
      const isOtherWorkflow = workflow.id !== this.workflow.id;
      if (isOtherWorkflow) {
        this.restartIn(workflow);
      }
      this.goToStep(stepId);
    }

    // slot 0 is where the panel first opens in a workflow: the overview, else step 1
    restartIn(workflow) {
      this.workflow = workflow;
      this.slotIndex = 0;
      this.selection = null;
      this.isStepsCardCollapsed = false;
      this.frontCard = STEPS_CARD;
    }

    /** Makes step `stepId` the slot and expands the steps card, opening a hidden panel; an unknown step changes nothing. */
    goToStep(stepId) {
      const stepIndex = this.workflow.steps.findIndex((step) => step.id === stepId);
      const isUnknownStep = stepIndex === -1;
      if (isUnknownStep) {
        return;
      }
      this.openPanel();
      this.slotIndex = this.overviewSlotCount() + stepIndex;
      this.expandStepsCard();
    }

    /** Moves the slot `delta` slots on while the panel is visible, never past the first or the last, keeping the steps card collapsed or expanded. */
    moveSlotBy(delta) {
      const isPanelHidden = !this.panel.isVisible;
      if (isPanelHidden) {
        return;
      }
      const lastSlotIndex = this.slotCount() - 1;
      const movedSlotIndex = this.slotIndex + delta;
      const slotIndexFromFirst = Math.max(0, movedSlotIndex);
      this.slotIndex = Math.min(lastSlotIndex, slotIndexFromFirst);
    }

    /** Shows slot 0 again, where the workflow starts, so nothing dims: the overview, or, without one, a hidden panel. */
    returnToStart() {
      this.slotIndex = 0;
      const hasOverview = this.overviewSlotCount() === 1;
      if (hasOverview) {
        return;
      }
      this.hidePanel();
    }

    openPanel() {
      this.panel = this.panel.opened();
    }

    hidePanel() {
      const hiddenPanel = this.panel.hidden();
      this.changePanelTo(hiddenPanel);
    }

    tapHandle() {
      const tappedPanel = this.panel.handleTapped();
      this.changePanelTo(tappedPanel);
    }

    snapPanelNear(heightPx, boxHeightPx) {
      const snappedPanel = this.panel.snappedNear(heightPx, boxHeightPx);
      this.changePanelTo(snappedPanel);
    }

    /** Shows `panel` in place of the current one; hiding the panel clears the selected item. */
    changePanelTo(panel) {
      this.panel = panel;
      const isPanelHidden = !panel.isVisible;
      if (isPanelHidden) {
        this.clearSelection();
      }
    }

    /** Turns the panel into the kind an on-screen box `boxWidth` px wide takes, a visible one staying visible. */
    fitPanelTo(boxWidth) {
      const panelKind = panelKindFor(boxWidth);
      const isSameKind = this.panel instanceof panelKind;
      if (isSameKind) {
        return;
      }
      this.panel = panelKind.like(this.panel);
    }

    /** Selects `item`, opening a hidden panel, expanding a peeking sheet and expanding the component card. */
    select(item) {
      this.openPanel();
      this.panel = this.panel.showingCardBodies();
      this.selection = item;
      this.frontCard = COMPONENT_CARD;
    }

    /** Clears the selected item, which collapses the component card. */
    clearSelection() {
      this.selection = null;
      this.frontCard = STEPS_CARD;
    }

    /** Collapses the steps card while its body shows, which stops its step dimming; else expands it. */
    toggleStepsCard() {
      const parts = this.panel.partsShowing(this);
      const isStepsBodyShowing = parts.isStepsCardExpanded;
      if (isStepsBodyShowing) {
        this.isStepsCardCollapsed = true;
        this.frontCard = COMPONENT_CARD;
        return;
      }
      this.expandStepsCard();
    }

    /** Expands the steps card and brings it to the front, out of a peeking sheet too. */
    expandStepsCard() {
      this.isStepsCardCollapsed = false;
      this.frontCard = STEPS_CARD;
      this.panel = this.panel.showingCardBodies();
    }

    /** Tells whether the component card's body shows. */
    isComponentBodyShowing() {
      const parts = this.panel.partsShowing(this);
      return parts.isComponentCardExpanded;
    }

    /** Brings the component card to the front; with nothing selected, first selects the component `itemOfStepItem` finds for the first item of the shown step, step 1's on the overview. */
    expandComponentCard(itemOfStepItem) {
      const hasSelection = this.selection !== null;
      if (hasSelection) {
        this.frontCard = COMPONENT_CARD;
        this.panel = this.panel.showingCardBodies();
        return;
      }
      const shownStep = this.stepAt(this.slotIndex);
      const firstStep = this.workflow.steps[0];
      const describedStep = shownStep ?? firstStep;
      const firstStepItem = describedStep === undefined ? undefined : describedStep.items[0];
      const item = firstStepItem === undefined ? null : itemOfStepItem(firstStepItem);
      if (item === null) {
        return;
      }
      this.select(item);
    }

    /** Tells whether a group is dimmed: never when selected; else, while a step dims, when it takes no part in that step; else when it is unrelated to the hovered group. */
    dimsGroup(groupFacts) {
      if (groupFacts.isSelected) {
        return false;
      }
      const dimmingStep = this.dimmingStep();
      const isStepDimming = dimmingStep !== null;
      if (isStepDimming) {
        const isOutsideStep = !groupFacts.stepIds.includes(dimmingStep.id);
        return isOutsideStep;
      }
      return groupFacts.isUnrelatedToHovered;
    }

    /** Returns the step dimming the scene: the shown slot's while the panel is visible and the steps card expanded; null on the overview. */
    dimmingStep() {
      const isPanelHidden = !this.panel.isVisible;
      const isDimmingOff = isPanelHidden || this.isStepsCardCollapsed;
      if (isDimmingOff) {
        return null;
      }
      return this.stepAt(this.slotIndex);
    }

    slotCount() {
      return this.overviewSlotCount() + this.workflow.steps.length;
    }

    overviewSlotCount() {
      const hasOverview = typeof this.workflow.description === 'string';
      return hasOverview ? 1 : 0;
    }

    /** Returns the step at `slotIndex`, null for the overview. */
    stepAt(slotIndex) {
      const stepIndex = slotIndex - this.overviewSlotCount();
      const step = this.workflow.steps[stepIndex];
      return step ?? null;
    }
  }

  /** Picks the panel kind for an on-screen box `boxWidth` px wide: a side panel from SIDE_PANEL_MIN_BOX_WIDTH_PX on, else a sheet. */
  function panelKindFor(boxWidth) {
    const isNarrowBox = boxWidth < SIDE_PANEL_MIN_BOX_WIDTH_PX;
    return isNarrowBox ? Sheet : SidePanel;
  }

  /** Holds the side panel's state, hidden or open, and frames it along the box's right edge: a tab while hidden, the full height while open. */
  class SidePanel {
    static hidden() {
      return new SidePanel(false);
    }

    static open() {
      return new SidePanel(true);
    }

    /** Turns `panel` into a side panel, open when it is visible. */
    static like(panel) {
      return panel.isVisible ? SidePanel.open() : SidePanel.hidden();
    }

    constructor(isOpen) {
      this.isVisible = isOpen;
      this.stateName = isOpen ? PANEL_OPEN : PANEL_HIDDEN;
    }

    opened() {
      return SidePanel.open();
    }

    hidden() {
      return SidePanel.hidden();
    }

    // an open side panel always shows its cards' bodies
    showingCardBodies() {
      return this;
    }

    // a side panel has no handle
    handleTapped() {
      return this;
    }

    snappedNear(_heightPx, _boxHeightPx) {
      return this;
    }

    /** Tells which parts show for `cards`: the tab while hidden; the resizing edge, both cards and, between two expanded cards, their divider while open, the steps card expanded unless collapsed, the component card while an item is selected. */
    partsShowing(cards) {
      const isOpen = this.isVisible;
      const hasSelection = cards.selection !== null;
      const isStepsCardExpanded = !cards.isStepsCardCollapsed;
      return {
        kindClass: 'svugy-panel-side',
        hasTab: !isOpen,
        hasHandle: false,
        hasEdge: isOpen,
        hasCardDivider: isOpen,
        hasStepsCard: isOpen,
        hasComponentCard: isOpen,
        isStepsCardExpanded,
        isComponentCardExpanded: hasSelection,
        isHandleExpanded: false,
      };
    }

    // hidden: the tab, centred on the right edge; open: min(W_max, ρ·B_w) wide, the box's full height
    frameRectIn(box, _measureContentHeight, preferredWidthPx) {
      const isHidden = !this.isVisible;
      if (isHidden) {
        const tabX = box.width - SIDE_PANEL_TAB_WIDTH_PX;
        const tabY = (box.height - SIDE_PANEL_TAB_HEIGHT_PX) / 2;
        return { x: tabX, y: tabY, width: SIDE_PANEL_TAB_WIDTH_PX, height: SIDE_PANEL_TAB_HEIGHT_PX, coveredRight: 0, coveredBottom: 0 };
      }
      const width = SidePanel.widthIn(box, preferredWidthPx);
      return { x: box.width - width, y: 0, width, height: box.height, coveredRight: width, coveredBottom: 0 };
    }

    // w = clamp(w_pref, w_min, σ_drag·B_w), else min(w_max, σ·B_w)
    /** Sizes the open side panel for `box`: the width the reader dragged it to, within its bounds, else the default. */
    static widthIn(box, preferredWidthPx) {
      const isDefaultWidth = preferredWidthPx === null;
      if (isDefaultWidth) {
        const boxShareWidth = SIDE_PANEL_MAX_BOX_SHARE * box.width;
        return Math.min(SIDE_PANEL_MAX_WIDTH_PX, boxShareWidth);
      }
      const maxWidth = SIDE_PANEL_MAX_DRAGGED_BOX_SHARE * box.width;
      const widthAboveMin = Math.max(SIDE_PANEL_MIN_WIDTH_PX, preferredWidthPx);
      return Math.min(maxWidth, widthAboveMin);
    }
  }

  /** Holds the sheet's state, hidden, peek or expanded, and frames it along the box's bottom edge: the handle while hidden, else its content's height. */
  class Sheet {
    static hidden() {
      return new Sheet(PANEL_HIDDEN);
    }

    static peek() {
      return new Sheet(PANEL_PEEK);
    }

    static expanded() {
      return new Sheet(PANEL_EXPANDED);
    }

    /** Turns `panel` into a sheet, expanded when it is visible. */
    static like(panel) {
      return panel.isVisible ? Sheet.expanded() : Sheet.hidden();
    }

    /** Frames a sheet `heightPx` tall along the box's bottom edge. */
    static frameRectAtHeight(box, heightPx) {
      return { x: 0, y: box.height - heightPx, width: box.width, height: heightPx, coveredRight: 0, coveredBottom: heightPx };
    }

    constructor(stateName) {
      this.stateName = stateName;
      this.isVisible = stateName !== PANEL_HIDDEN;
    }

    /** Opens a hidden sheet expanded; a visible one stays as it is. */
    opened() {
      return this.isVisible ? this : Sheet.expanded();
    }

    hidden() {
      return Sheet.hidden();
    }

    /** Expands a peeking sheet, the only visible state hiding the cards' bodies. */
    showingCardBodies() {
      const isPeek = this.stateName === PANEL_PEEK;
      return isPeek ? Sheet.expanded() : this;
    }

    /** Taps the handle: hidden → peek → expanded, expanded → peek. */
    handleTapped() {
      const isPeek = this.stateName === PANEL_PEEK;
      return isPeek ? Sheet.expanded() : Sheet.peek();
    }

    // argmin over resting heights: hidden = handle, peek = handle + one line, expanded = σ·B_h
    snappedNear(heightPx, boxHeightPx) {
      const restingSheets = [
        [Sheet.hidden(), SHEET_HANDLE_HEIGHT_PX],
        [Sheet.peek(), SHEET_HANDLE_HEIGHT_PX + SHEET_LINE_HEIGHT_PX],
        [Sheet.expanded(), SHEET_MAX_BOX_SHARE * boxHeightPx],
      ];
      const distances = restingSheets.map(([sheet, restingHeight]) => {
        const distance = Math.abs(heightPx - restingHeight);
        return [sheet, distance];
      });
      const [nearestSheet] = distances.reduce((nearest, candidate) => (candidate[1] < nearest[1] ? candidate : nearest));
      return nearestSheet;
    }

    /** Tells which parts show for `cards`: the handle always; the steps card while visible, down to its line on peek; once expanded, the body of at most one expanded card, the front one, the other down to its line. */
    partsShowing(cards) {
      const isExpanded = this.stateName === PANEL_EXPANDED;
      const hasSelection = cards.selection !== null;
      const isStepsCardOpen = !cards.isStepsCardCollapsed;
      const isComponentInFront = cards.frontCard === COMPONENT_CARD;
      const isComponentBodyInFront = hasSelection && isComponentInFront;
      const isStepsCardExpanded = isExpanded && isStepsCardOpen && !isComponentBodyInFront;
      const isComponentCardExpanded = isExpanded && hasSelection && (isComponentInFront || !isStepsCardOpen);
      return {
        kindClass: 'svugy-panel-sheet',
        hasTab: false,
        hasHandle: true,
        hasEdge: false,
        hasCardDivider: false,
        hasStepsCard: this.isVisible,
        hasComponentCard: isExpanded,
        isStepsCardExpanded,
        isComponentCardExpanded,
        isHandleExpanded: isExpanded,
      };
    }

    // hidden: the handle, centred on the bottom edge; else the content's height, at most σ·B_h
    frameRectIn(box, measureContentHeight) {
      const isHidden = !this.isVisible;
      if (isHidden) {
        const handleX = (box.width - SHEET_HANDLE_TAB_WIDTH_PX) / 2;
        const handleY = box.height - SHEET_HANDLE_HEIGHT_PX;
        return { x: handleX, y: handleY, width: SHEET_HANDLE_TAB_WIDTH_PX, height: SHEET_HANDLE_HEIGHT_PX, coveredRight: 0, coveredBottom: 0 };
      }
      const maxHeight = SHEET_MAX_BOX_SHARE * box.height;
      const contentHeight = measureContentHeight(box.width, maxHeight);
      return Sheet.frameRectAtHeight(box, contentHeight);
    }
  }

  /** Marks the shown layer's groups `data-svugy-dimmed` and `data-svugy-selected` as the reader state decides, given the hovered or focused group. */
  class SceneMarks {
    constructor(svg, state) {
      const staleMarks = svg.querySelectorAll(`${DIMMED_SELECTOR},${SELECTED_SELECTOR}`);
      staleMarks.forEach((node) => {
        node.removeAttribute(DIMMED);
        node.removeAttribute(SELECTED);
      });
      this.state = state;
      this.hoveredGroup = null;
      this.dimmedGroups = [];
      this.selectedGroups = [];
    }

    /** Records the hovered or focused group, null for none; tells whether it changed. */
    hover(group) {
      const isHoverChanged = group !== this.hoveredGroup;
      this.hoveredGroup = group;
      return isHoverChanged;
    }

    paint(shownLayer, deepestLayer) {
      this.clear();
      const selection = this.state.selection;
      this.selectedGroups = selection === null ? [] : selection.groupsIn(shownLayer, deepestLayer);
      const selectedGroupSet = new Set(this.selectedGroups);
      const hoveredRelatives = this.hoveredRelativesIn(shownLayer);
      const dimmableGroups = Contract.dimmableGroupsOf(shownLayer);
      this.dimmedGroups = dimmableGroups.filter((group) => {
        const stepIds = Contract.stepIdsOf(group);
        const isSelected = selectedGroupSet.has(group);
        const isRelatedToHovered = hoveredRelatives === null ? true : hoveredRelatives.has(group);
        const groupFacts = { stepIds, isSelected, isUnrelatedToHovered: !isRelatedToHovered };
        return this.state.dimsGroup(groupFacts);
      });
      this.selectedGroups.forEach((group) => group.setAttribute(SELECTED, ''));
      this.dimmedGroups.forEach((group) => group.setAttribute(DIMMED, ''));
    }

    /** Collects the hovered group's relatives in `shownLayer`; null when no element or edge group of it is hovered. */
    hoveredRelativesIn(shownLayer) {
      const group = this.hoveredGroup;
      if (group === null) {
        return null;
      }
      const isGroupInShownLayer = shownLayer.contains(group);
      const isStub = group.hasAttribute(STUB);
      const isDimmingHover = isGroupInShownLayer && !isStub;
      return isDimmingHover ? Relatives.of(group, shownLayer) : null;
    }

    clear() {
      this.dimmedGroups.forEach((group) => group.removeAttribute(DIMMED));
      this.selectedGroups.forEach((group) => group.removeAttribute(SELECTED));
      this.dimmedGroups = [];
      this.selectedGroups = [];
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

  /** Resolves a description link's `#TARGET`: a step of any workflow as `WORKFLOW/STEP`, else an element or written-edge group of the shown workflow's layer D; null when it names nothing. */
  class LinkTargets {
    constructor(workflows, navigator) {
      const workflowEntries = workflows.map((workflow) => {
        const workflowId = workflow.getAttribute(WORKFLOW);
        const workflowDetails = Workflows.detailsOf(workflow);
        return [workflowId, workflowDetails];
      });
      this.workflowDetailsById = new Map(workflowEntries);
      this.navigator = navigator;
    }

    resolve(target) {
      const stepTarget = this.stepTargetOf(target);
      if (stepTarget !== null) {
        return stepTarget;
      }
      const deepestLayer = this.navigator.deepestLayer();
      const group = Contract.groupTargetedBy(deepestLayer, target);
      return group === null ? null : { group };
    }

    stepTargetOf(target) {
      const targetName = target.slice(TARGET_LINK_PREFIX.length);
      const stepTargetMatch = STEP_TARGET_PATTERN.exec(targetName);
      if (stepTargetMatch === null) {
        return null;
      }
      const [_targetName, workflowId, stepId] = stepTargetMatch;
      const workflowDetails = this.workflowDetailsById.get(workflowId);
      const steps = workflowDetails === undefined ? [] : workflowDetails.steps;
      const isStepOfWorkflow = steps.some((step) => step.id === stepId);
      return isStepOfWorkflow ? { workflowId, stepId } : null;
    }
  }

  /** Lets the reader pick the shown workflow from a menu below the heading, listing every workflow's title and subtitle. */
  class WorkflowMenu {
    constructor(svg, workflows, navigator, reader) {
      this.svg = svg;
      this.workflows = workflows;
      this.navigator = navigator;
      this.reader = reader;
      this.list = createXhtmlElement('div', { class: 'svugy-workflows', role: 'listbox', 'aria-label': 'workflows' });
      this.frame = createSvgElement('foreignObject', { class: 'svugy-workflows-frame', width: WORKFLOW_MENU_WIDTH_PX, height: 0 });
      this.frame.appendChild(this.list);
      this.pin = createSvgElement('g', { class: 'svugy-workflows-pin' });
      this.pin.appendChild(this.frame);
      isolateOverlayEvents(this.pin);
      this.pin.addEventListener('keydown', (event) => this.followKey(event));
      this.isOpen = false;
    }

    /** Marks every heading as the menu's trigger, closes the menu on a press outside it and keeps it under the shown heading. */
    bind() {
      const headings = this.svg.querySelectorAll(HEADING_SELECTOR);
      headings.forEach((heading) => WorkflowMenu.markAsTrigger(heading));
      // capture, so a press on an overlay that keeps its events still closes the menu
      this.svg.addEventListener('pointerdown', (event) => this.closeOnPressOutside(event), true);
      const viewBoxObserver = new MutationObserver(() => this.place());
      viewBoxObserver.observe(this.svg, { attributes: true, attributeFilter: ['viewBox'] });
      const resizeObserver = new ResizeObserver(() => this.place());
      resizeObserver.observe(this.svg);
    }

    static markAsTrigger(heading) {
      heading.setAttribute('tabindex', '0');
      heading.setAttribute('role', 'button');
      heading.setAttribute('aria-haspopup', 'listbox');
      heading.setAttribute('aria-expanded', 'false');
      heading.setAttribute('aria-label', 'choose workflow');
      const title = heading.querySelector('text');
      if (title === null) {
        return;
      }
      const titleX = Number(title.getAttribute('x'));
      const writtenLength = Number(title.getAttribute('textLength'));
      const hasWrittenLength = writtenLength > 0;
      const titleLength = hasWrittenLength ? writtenLength : title.getComputedTextLength();
      const caret = title.cloneNode(false);
      caret.removeAttribute('textLength');
      caret.removeAttribute('lengthAdjust');
      caret.setAttribute('x', String(titleX + titleLength + HEADING_CARET_GAP));
      caret.textContent = CARET;
      title.after(caret);
    }

    toggle() {
      if (this.isOpen) {
        this.close();
        return;
      }
      this.open();
    }

    open() {
      const shownWorkflow = this.navigator.shownWorkflow;
      const options = this.workflows.map((workflow) => this.optionChoosing(workflow, shownWorkflow));
      this.list.replaceChildren(...options);
      this.svg.appendChild(this.pin);
      this.isOpen = true;
      this.markTriggersExpanded(true);
      const svgBox = this.svg.getBoundingClientRect();
      this.frame.setAttribute('height', String(svgBox.height));
      const listHeight = this.list.offsetHeight;
      this.frame.setAttribute('height', String(listHeight));
      this.place();
      const shownIndex = this.workflows.indexOf(shownWorkflow);
      const shownOption = options[shownIndex] ?? options[0];
      shownOption.focus({ preventScroll: true });
    }

    optionChoosing(workflow, shownWorkflow) {
      const details = Workflows.detailsOf(workflow);
      const isShownWorkflow = workflow === shownWorkflow;
      const option = createXhtmlElement('div', {
        class: 'svugy-workflows-option',
        role: 'option',
        tabindex: '-1',
        'aria-selected': String(isShownWorkflow),
      });
      const titleText = details.title ?? workflow.getAttribute(WORKFLOW);
      const title = createTextElement('div', 'svugy-workflows-title', titleText);
      const subtitle = createTextElement('div', 'svugy-workflows-subtitle', details.subtitle);
      const parts = [title, subtitle].filter((part) => part !== null);
      option.append(...parts);
      option.addEventListener('click', () => this.choose(workflow));
      return option;
    }

    choose(workflow) {
      this.close();
      const isShownWorkflow = workflow === this.navigator.shownWorkflow;
      if (isShownWorkflow) {
        return;
      }
      this.reader.switchWorkflow(workflow);
    }

    close() {
      const isClosed = !this.isOpen;
      if (isClosed) {
        return;
      }
      const wasFocusInside = this.pin.contains(document.activeElement);
      this.pin.remove();
      this.isOpen = false;
      this.markTriggersExpanded(false);
      if (wasFocusInside) {
        const shownHeading = Contract.headingOf(this.navigator.shownLayer);
        shownHeading?.focus({ preventScroll: true });
      }
    }

    closeOnPressOutside(event) {
      const isInsideMenu = this.pin.contains(event.target);
      const heading = Interactions.headingAt(event.target);
      const isOnHeading = heading !== null;
      const isPressOutside = !isInsideMenu && !isOnHeading;
      if (isPressOutside) {
        this.close();
      }
    }

    // ↑ and ↓ move between options, Enter and Space choose, Escape closes; none reaches the diagram
    followKey(event) {
      event.stopPropagation();
      const options = Array.from(this.list.children);
      const focusedIndex = options.indexOf(document.activeElement);
      const key = event.key;
      if (key === 'Escape') {
        this.close();
        return;
      }
      const isChoosingKey = key === 'Enter' || key === ' ';
      if (isChoosingKey) {
        event.preventDefault();
        options[focusedIndex]?.click();
        return;
      }
      const focusDelta = OPTION_FOCUS_DELTA_BY_KEY.get(key);
      if (focusDelta === undefined) {
        return;
      }
      event.preventDefault();
      const movedIndex = focusedIndex + focusDelta;
      const clampedIndex = Math.min(options.length - 1, Math.max(0, movedIndex));
      options[clampedIndex].focus({ preventScroll: true });
    }

    markTriggersExpanded(isExpanded) {
      const headings = this.svg.querySelectorAll(HEADING_SELECTOR);
      headings.forEach((heading) => heading.setAttribute('aria-expanded', String(isExpanded)));
    }

    // top-left = the shown heading's bottom-left, g pixels lower
    place() {
      const isClosed = !this.isOpen;
      if (isClosed) {
        return;
      }
      const shownHeading = Contract.headingOf(this.navigator.shownLayer);
      if (shownHeading === null) {
        return;
      }
      const headingBounds = shownHeading.getBoundingClientRect();
      const clientCorner = { x: headingBounds.left, y: headingBounds.bottom + WORKFLOW_MENU_HEADING_GAP_PX };
      pinOverlayAt(this.svg, this.pin, clientCorner);
    }
  }

  const OPTION_FOCUS_DELTA_BY_KEY = new Map([
    ['ArrowDown', 1],
    ['ArrowUp', -1],
  ]);

  /** Draws the reader state's description panel as its side panel or sheet strategy presents and frames it, and reports the strip it covers to the uncovered area. */
  class DescriptionPanel {
    constructor(svg, uncoveredArea, reader, linkTargets) {
      this.svg = svg;
      this.uncoveredArea = uncoveredArea;
      this.reader = reader;
      const resolveTarget = (target) => linkTargets.resolve(target);
      const followTarget = (target) => reader.followLink(target);
      const markdown = new SafeMarkdown(resolveTarget, followTarget);
      const goToStep = (stepId) => reader.apply((state) => state.goToStep(stepId));
      this.content = new CardContent(markdown, goToStep);
      this.previousButtons = [];
      this.nextButtons = [];
      this.root = createXhtmlElement('aside', { class: 'svugy-panel', 'aria-label': 'description' });
      const openPanel = () => reader.apply((state) => state.openPanel());
      this.tab = this.signButton(PREVIOUS_SIGN, 'show description', openPanel);
      this.tab.classList.add('svugy-panel-tab');
      this.handle = createXhtmlElement('div', { class: 'svugy-panel-handle', role: 'button', tabindex: '0', 'aria-label': 'description sheet' });
      const handleBar = createXhtmlElement('div', { class: 'svugy-panel-handle-bar' });
      this.handle.appendChild(handleBar);
      this.stepsLine = this.lineShowingSlot();
      this.stepsBody = createXhtmlElement('div', { class: 'svugy-panel-body' });
      this.stepsCard = createXhtmlElement('section', { class: 'svugy-panel-card svugy-panel-steps', 'aria-label': 'steps' });
      this.stepsCard.append(this.stepsLine, this.stepsBody);
      this.componentLine = this.lineShowingItem();
      this.componentBody = createXhtmlElement('div', { class: 'svugy-panel-body' });
      this.componentCard = createXhtmlElement('section', { class: 'svugy-panel-card svugy-panel-component', 'aria-label': 'component' });
      this.componentCard.append(this.componentLine, this.componentBody);
      this.edge = createXhtmlElement('div', { class: 'svugy-panel-edge', role: 'separator', tabindex: '0', 'aria-orientation': 'vertical', 'aria-label': 'resize description' });
      this.hideTab = this.signButton(HIDE_SIGN, 'hide description', (event) => this.hideOnKeyboardActivation(event));
      this.hideTab.classList.add('svugy-panel-hide');
      this.edge.appendChild(this.hideTab);
      this.cardDivider = createXhtmlElement('div', { class: 'svugy-panel-divider', role: 'separator', tabindex: '0', 'aria-orientation': 'horizontal', 'aria-label': 'resize cards; double click toggles the component card' });
      this.root.append(this.tab, this.handle, this.edge, this.stepsCard, this.cardDivider, this.componentCard);
      this.frame = createSvgElement('foreignObject', { class: 'svugy-panel-frame', width: 0, height: 0 });
      this.frame.appendChild(this.root);
      this.pin = createSvgElement('g', { class: 'svugy-panel-pin' });
      this.pin.appendChild(this.frame);
      isolateOverlayEvents(this.pin);
      this.sheetHandle = new SheetHandle(svg, this.handle, this, reader);
      this.sheetDragHeight = null;
      this.sidePanelWidthPx = null;
      this.stepsCardShare = null;
      this.resizeStart = null;
      this.bindResizers();
      this.frameRect = { x: 0, y: 0, width: 0, height: 0 };
      this.renderedWorkflow = null;
      this.renderedSlotIndex = null;
      this.renderedItem = null;
    }

    /** Attaches the panel to the diagram, keeps it pinned as the view moves, and refits it to the box, as a side panel or a sheet, whenever the box resizes. */
    bind() {
      this.svg.appendChild(this.pin);
      const viewBoxObserver = new MutationObserver(() => this.place());
      viewBoxObserver.observe(this.svg, { attributes: true, attributeFilter: ['viewBox'] });
      const resizeObserver = new ResizeObserver(() => this.refitToBox());
      resizeObserver.observe(this.svg);
    }

    refitToBox() {
      const box = this.svg.getBoundingClientRect();
      const isUnlaidOut = box.width === 0 || box.height === 0;
      if (isUnlaidOut) {
        return;
      }
      const boxWidth = box.width;
      this.reader.apply((state) => state.fitPanelTo(boxWidth));
    }

    /** The steps card's line: a toggle showing `i/N TITLE` that collapses or expands the steps card, then ‹ ›. */
    lineShowingSlot() {
      this.stepsLineLabel = createXhtmlElement('span', { class: 'svugy-panel-line-label' });
      this.stepsLineTitle = createXhtmlElement('span', { class: 'svugy-panel-line-title' });
      const toggleStepsCard = () => this.reader.apply((state) => state.toggleStepsCard());
      this.stepsLineToggle = DescriptionPanel.lineToggle(this.stepsLineLabel, this.stepsLineTitle, toggleStepsCard);
      const slotButtons = this.slotButtons();
      const line = createXhtmlElement('div', { class: 'svugy-panel-line' });
      line.append(this.stepsLineToggle, ...slotButtons);
      return line;
    }

    /** Lets the reader drag, or move with arrow keys, the side panel's edge to resize the panel and the divider to resize the steps card's share of the two cards. */
    bindResizers() {
      const rememberResizeStart = () => this.rememberResizeStart();
      const resizeSidePanelBy = (draggedX) => this.resizeSidePanelBy(draggedX);
      const hideOnTabTap = (pressTarget) => this.hideOnTabTap(pressTarget);
      new DragGrip(this.edge, { start: rememberResizeStart, drag: resizeSidePanelBy, tap: hideOnTabTap });
      const hidePanel = () => this.reader.apply((state) => state.hidePanel());
      this.edge.addEventListener('dblclick', hidePanel);
      this.edge.addEventListener('keydown', (event) => DescriptionPanel.actOnActivationKey(event, hidePanel));
      const resizeStepsCardBy = (_draggedX, draggedY) => this.resizeStepsCardBy(draggedY);
      new DragGrip(this.cardDivider, { start: rememberResizeStart, drag: resizeStepsCardBy });
      const toggleComponentCard = () => this.reader.toggleComponentCard();
      this.cardDivider.addEventListener('dblclick', toggleComponentCard);
      const widenSidePanelBy = (keyStepPx) => this.resizeSidePanelBy(-keyStepPx);
      this.edge.addEventListener('keydown', (event) => this.resizeOnArrowKey(event, 'ArrowLeft', 'ArrowRight', widenSidePanelBy));
      const growStepsCardBy = (keyStepPx) => this.resizeStepsCardBy(keyStepPx);
      this.cardDivider.addEventListener('keydown', (event) => this.resizeOnArrowKey(event, 'ArrowDown', 'ArrowUp', growStepsCardBy));
      this.cardDivider.addEventListener('keydown', (event) => DescriptionPanel.actOnActivationKey(event, toggleComponentCard));
    }

    // a pointer press on the tab is the edge's: a drag resizes, a tap hides, as does a double click anywhere on the edge
    hideOnTabTap(pressTarget) {
      const isTabPress = this.hideTab.contains(pressTarget);
      if (isTabPress) {
        this.reader.apply((state) => state.hidePanel());
      }
    }

    // a pointer tap on the tab reaches hideOnTabTap; only Enter or Space click it with no pointer, detail 0
    hideOnKeyboardActivation(event) {
      const isKeyboardActivation = event.detail === 0;
      if (isKeyboardActivation) {
        this.reader.apply((state) => state.hidePanel());
      }
    }

    // Enter or Space on a focused resizer acts like its double click, and never reaches the diagram
    static actOnActivationKey(event, act) {
      const isActivationKey = event.key === 'Enter' || event.key === ' ';
      if (isActivationKey) {
        event.preventDefault();
        event.stopPropagation();
        act();
      }
    }

    // a handled arrow key never reaches the diagram, whose ← and → change step
    resizeOnArrowKey(event, growKey, shrinkKey, resizeBy) {
      const key = event.key;
      const isGrowKey = key === growKey;
      const isShrinkKey = key === shrinkKey;
      const isOtherKey = !isGrowKey && !isShrinkKey;
      if (isOtherKey) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      this.rememberResizeStart();
      const keyStepPx = isGrowKey ? GRIP_KEY_STEP_PX : -GRIP_KEY_STEP_PX;
      resizeBy(keyStepPx);
    }

    rememberResizeStart() {
      const panelWidth = this.frameRect.width;
      const stepsCardHeight = this.stepsCard.offsetHeight;
      const cardsHeight = stepsCardHeight + this.componentCard.offsetHeight;
      this.resizeStart = { panelWidth, stepsCardHeight, cardsHeight };
    }

    /** Widens the side panel by the pixels its left edge was dragged leftwards, within its bounds, and lays it out again. */
    resizeSidePanelBy(draggedX) {
      const box = this.svg.getBoundingClientRect();
      const draggedWidth = this.resizeStart.panelWidth - draggedX;
      this.sidePanelWidthPx = SidePanel.widthIn(box, draggedWidth);
      this.layOut();
    }

    // s = clamp((h_s + Δy) / (h_s + h_c), s_min, 1 − s_min)
    /** Gives the steps card the share of the two cards' height its divider was dragged to, while both cards are expanded, and lays the cards out again. */
    resizeStepsCardBy(draggedY) {
      const state = this.reader.state;
      const shownParts = state.panel.partsShowing(state);
      const isSplitHidden = !DescriptionPanel.areBothCardsExpandedIn(shownParts);
      if (isSplitHidden) {
        return;
      }
      const resizeStart = this.resizeStart;
      const draggedShare = (resizeStart.stepsCardHeight + draggedY) / resizeStart.cardsHeight;
      const maxShare = 1 - STEPS_CARD_MIN_SHARE;
      const shareAboveMin = Math.max(STEPS_CARD_MIN_SHARE, draggedShare);
      this.stepsCardShare = Math.min(maxShare, shareAboveMin);
      this.renderParts(state);
    }

    /** The component card's line: a toggle showing the selected item's kind and name, or `COMPONENTS`, that collapses or expands the component card, then ✕. */
    lineShowingItem() {
      this.componentLineLabel = createXhtmlElement('span', { class: 'svugy-panel-line-label' });
      this.componentLineTitle = createXhtmlElement('span', { class: 'svugy-panel-line-title' });
      const toggleComponentCard = () => this.reader.toggleComponentCard();
      this.componentLineToggle = DescriptionPanel.lineToggle(this.componentLineLabel, this.componentLineTitle, toggleComponentCard);
      const deselect = () => this.reader.deselect();
      this.clearButton = this.signButton(CLEAR_SIGN, 'clear selection', deselect);
      const line = createXhtmlElement('div', { class: 'svugy-panel-line' });
      line.append(this.componentLineToggle, this.clearButton);
      return line;
    }

    static lineToggle(label, title, toggleCard) {
      const toggle = createXhtmlElement('button', { type: 'button', class: 'svugy-panel-line-toggle', 'aria-expanded': 'false' });
      toggle.append(label, title);
      toggle.addEventListener('click', toggleCard);
      return toggle;
    }

    slotButtons() {
      const showPreviousSlot = () => this.reader.apply((state) => state.moveSlotBy(-1));
      const showNextSlot = () => this.reader.apply((state) => state.moveSlotBy(1));
      const previousButton = this.signButton(PREVIOUS_SIGN, 'previous step', showPreviousSlot);
      const nextButton = this.signButton(NEXT_SIGN, 'next step', showNextSlot);
      this.previousButtons.push(previousButton);
      this.nextButtons.push(nextButton);
      return [previousButton, nextButton];
    }

    signButton(sign, label, activate) {
      const button = createXhtmlElement('button', { type: 'button', class: 'svugy-panel-sign', 'aria-label': label });
      button.textContent = sign;
      button.addEventListener('click', activate);
      return button;
    }

    render(deepestLayer) {
      const state = this.reader.state;
      this.renderSlot(state);
      this.renderItem(state, deepestLayer);
      this.renderedWorkflow = state.workflow;
      const wasFocusInPanel = this.pin.contains(document.activeElement);
      this.renderParts(state);
      this.layOut();
      this.keepKeyboardFocusInDiagram(wasFocusInPanel);
    }

    // a control hidden while focused hands focus to the root, so ← → and Escape keep reaching the diagram
    keepKeyboardFocusInDiagram(wasFocusInPanel) {
      const focusedNode = document.activeElement;
      const isFocusInPanel = this.pin.contains(focusedNode);
      const hiddenAncestor = isFocusInPanel ? focusedNode.closest('[hidden]') : null;
      const isFocusHidden = hiddenAncestor !== null;
      const isFocusDropped = wasFocusInPanel && !isFocusInPanel;
      const isFocusLost = isFocusHidden || isFocusDropped;
      if (isFocusLost) {
        this.svg.focus({ preventScroll: true });
      }
    }

    renderSlot(state) {
      const slotIndex = state.slotIndex;
      const step = state.stepAt(slotIndex);
      const isOverview = step === null;
      const stepCount = state.workflow.steps.length;
      const stepNumber = state.workflow.steps.indexOf(step) + 1;
      this.stepsLineLabel.textContent = isOverview ? 'Overview' : `${stepNumber}/${stepCount}`;
      const slotTitle = isOverview ? state.workflow.title : step.title;
      this.stepsLineTitle.textContent = slotTitle ?? '';
      const isFirstSlot = slotIndex === 0;
      const isLastSlot = slotIndex >= state.slotCount() - 1;
      this.previousButtons.forEach((button) => { button.disabled = isFirstSlot; });
      this.nextButtons.forEach((button) => { button.disabled = isLastSlot; });
      const isSameSlot = state.workflow === this.renderedWorkflow && slotIndex === this.renderedSlotIndex;
      if (isSameSlot) {
        return;
      }
      this.renderedSlotIndex = slotIndex;
      const slotNodes = this.content.ofSlot(state.workflow, step);
      this.stepsBody.replaceChildren(...slotNodes);
      this.stepsBody.scrollTop = 0;
    }

    renderItem(state, deepestLayer) {
      const item = state.selection;
      const hasItem = item !== null;
      const kindLabel = hasItem ? item.kindLabel : 'Components';
      this.componentLineLabel.textContent = kindLabel;
      const itemName = hasItem ? item.nameIn(deepestLayer) : '';
      this.componentLineTitle.textContent = itemName;
      showIf(this.clearButton, hasItem);
      const isSameItem = state.workflow === this.renderedWorkflow && item === this.renderedItem;
      if (isSameItem) {
        return;
      }
      this.renderedItem = item;
      const itemNodes = this.content.ofItem(item, state.workflow, deepestLayer);
      this.componentBody.replaceChildren(...itemNodes);
      this.componentBody.scrollTop = 0;
    }

    // a dragged sheet presents as an expanded one
    renderParts(state) {
      const isDragging = this.sheetDragHeight !== null;
      const presentedPanel = isDragging ? Sheet.expanded() : state.panel;
      const parts = presentedPanel.partsShowing(state);
      this.root.className = `svugy-panel ${parts.kindClass} svugy-panel-${presentedPanel.stateName}`;
      this.root.classList.toggle('svugy-panel-component-expanded', parts.isComponentCardExpanded);
      this.stepsCard.classList.toggle('svugy-panel-card-expanded', parts.isStepsCardExpanded);
      this.componentCard.classList.toggle('svugy-panel-card-expanded', parts.isComponentCardExpanded);
      this.stepsLineToggle.setAttribute('aria-expanded', String(parts.isStepsCardExpanded));
      this.componentLineToggle.setAttribute('aria-expanded', String(parts.isComponentCardExpanded));
      this.handle.setAttribute('aria-expanded', String(parts.isHandleExpanded));
      showIf(this.tab, parts.hasTab);
      showIf(this.handle, parts.hasHandle);
      showIf(this.edge, parts.hasEdge);
      showIf(this.cardDivider, parts.hasCardDivider);
      this.splitCards(parts);
      showIf(this.stepsCard, parts.hasStepsCard);
      showIf(this.stepsLineTitle, !parts.isStepsCardExpanded);
      showIf(this.stepsBody, parts.isStepsCardExpanded);
      showIf(this.componentCard, parts.hasComponentCard);
      showIf(this.componentLineTitle, !parts.isComponentCardExpanded);
      showIf(this.componentBody, parts.isComponentCardExpanded);
    }

    /** Tells whether `parts` show both cards expanded beside the divider, the only time the divider splits them. */
    static areBothCardsExpandedIn(parts) {
      return parts.hasCardDivider && parts.isStepsCardExpanded && parts.isComponentCardExpanded;
    }

    /** Gives the steps card the share the reader dragged its divider to, while both cards are expanded; else the default split. */
    splitCards(parts) {
      const areBothCardsExpanded = DescriptionPanel.areBothCardsExpandedIn(parts);
      const isDefaultSplit = !areBothCardsExpanded || this.stepsCardShare === null;
      if (isDefaultSplit) {
        this.stepsCard.style.removeProperty('flex');
        this.stepsCard.style.removeProperty('max-height');
        return;
      }
      const sharePercent = this.stepsCardShare * 100;
      this.stepsCard.style.flex = `0 0 ${sharePercent}%`;
      this.stepsCard.style.maxHeight = 'none';
    }

    /** Sizes and places the panel's frame for the box it floats over and reports the strip it covers to the uncovered area. */
    layOut() {
      const box = this.svg.getBoundingClientRect();
      const isUnlaidOut = box.width === 0 || box.height === 0;
      if (isUnlaidOut) {
        return;
      }
      this.frameRect = this.frameRectIn(box);
      this.sizeFrameTo(this.frameRect);
      this.place();
      this.uncoveredArea.cover(this.frameRect.coveredRight, this.frameRect.coveredBottom);
    }

    frameRectIn(box) {
      const isDragging = this.sheetDragHeight !== null;
      if (isDragging) {
        return Sheet.frameRectAtHeight(box, this.sheetDragHeight);
      }
      const measureContentHeight = (width, maxHeight) => this.measureContentHeight(width, maxHeight);
      return this.reader.state.panel.frameRectIn(box, measureContentHeight, this.sidePanelWidthPx);
    }

    /** Lays the content out `width` px wide, at most `maxHeight` px tall, and returns its height. */
    measureContentHeight(width, maxHeight) {
      this.frame.setAttribute('width', String(width));
      this.frame.setAttribute('height', String(maxHeight));
      this.root.style.width = `${width}px`;
      this.root.style.removeProperty('height');
      this.root.style.maxHeight = `${maxHeight}px`;
      return this.root.offsetHeight;
    }

    sizeFrameTo(frameRect) {
      this.frame.setAttribute('width', String(frameRect.width));
      this.frame.setAttribute('height', String(frameRect.height));
      this.root.style.removeProperty('max-height');
      this.root.style.width = `${frameRect.width}px`;
      this.root.style.height = `${frameRect.height}px`;
    }

    place() {
      const box = this.svg.getBoundingClientRect();
      const clientCorner = { x: box.left + this.frameRect.x, y: box.top + this.frameRect.y };
      pinOverlayAt(this.svg, this.pin, clientCorner);
    }

    /** Shows the sheet as expanded at `heightPx` while its handle is dragged; null ends the preview. */
    previewSheetHeight(heightPx) {
      this.sheetDragHeight = heightPx;
      const state = this.reader.state;
      this.renderParts(state);
      this.layOut();
    }

    frameHeightPx() {
      return this.frameRect.height;
    }
  }

  /** Turns a tap on the sheet's handle into a handle tap, and a drag into a sheet following the pointer that snaps on release to the nearest of hidden, peek and expanded. */
  /** Turns a pointer press on `grip` into a drag once it moves DRAG_THRESHOLD_PX, reporting each move as the pixels dragged since the press, then a release; a press that never moves that far is a tap. */
  class DragGrip {
    constructor(grip, gestures) {
      this.grip = grip;
      this.gestures = gestures;
      this.press = null;
      grip.addEventListener('pointerdown', (event) => this.startPress(event));
      grip.addEventListener('pointermove', (event) => this.movePress(event));
      grip.addEventListener('pointerup', () => this.endPress());
      grip.addEventListener('pointercancel', () => this.endPress());
    }

    startPress(event) {
      const isOtherMouseButton = event.pointerType === 'mouse' && event.button !== 0;
      if (isOtherMouseButton) {
        return;
      }
      event.preventDefault();
      this.grip.setPointerCapture(event.pointerId);
      this.gestures.start?.();
      this.press = { target: event.target, startX: event.clientX, startY: event.clientY, isDragging: false };
    }

    movePress(event) {
      const press = this.press;
      if (press === null) {
        return;
      }
      const draggedX = event.clientX - press.startX;
      const draggedY = event.clientY - press.startY;
      const draggedDistance = Math.hypot(draggedX, draggedY);
      const isStillATap = !press.isDragging && draggedDistance < DRAG_THRESHOLD_PX;
      if (isStillATap) {
        return;
      }
      press.isDragging = true;
      this.gestures.drag(draggedX, draggedY);
    }

    endPress() {
      const press = this.press;
      if (press === null) {
        return;
      }
      this.press = null;
      if (press.isDragging) {
        this.gestures.release?.();
        return;
      }
      this.gestures.tap?.(press.target);
    }
  }

  /** Turns the sheet's handle into its controls: a tap or an activation key steps the sheet's state, a drag moves the sheet and snaps it to the nearest state on release. */
  class SheetHandle {
    constructor(svg, handle, panel, reader) {
      this.svg = svg;
      this.panel = panel;
      this.reader = reader;
      this.startHeight = 0;
      const gestures = {
        start: () => this.rememberStartHeight(),
        drag: (_draggedX, draggedY) => this.previewDraggedHeight(draggedY),
        release: () => this.snapReleasedSheet(),
        tap: () => this.tap(),
      };
      new DragGrip(handle, gestures);
      handle.addEventListener('keydown', (event) => this.tapOnActivationKey(event));
    }

    rememberStartHeight() {
      this.startHeight = this.panel.frameHeightPx();
    }

    // h = clamp(h₀ − Δy, handle height, σ·B_h)
    previewDraggedHeight(draggedY) {
      const box = this.svg.getBoundingClientRect();
      const maxHeight = SHEET_MAX_BOX_SHARE * box.height;
      const draggedHeight = this.startHeight - draggedY;
      const heightAboveHandle = Math.max(SHEET_HANDLE_HEIGHT_PX, draggedHeight);
      const height = Math.min(maxHeight, heightAboveHandle);
      this.panel.previewSheetHeight(height);
    }

    snapReleasedSheet() {
      const releasedHeight = this.panel.frameHeightPx();
      this.panel.previewSheetHeight(null);
      const box = this.svg.getBoundingClientRect();
      const boxHeight = box.height;
      this.reader.apply((state) => state.snapPanelNear(releasedHeight, boxHeight));
    }

    tap() {
      this.reader.apply((state) => state.tapHandle());
    }

    tapOnActivationKey(event) {
      const isActivationKey = event.key === 'Enter' || event.key === ' ';
      if (isActivationKey) {
        event.preventDefault();
        this.tap();
      }
    }
  }

  /** Builds the panel cards' XHTML: a slot's title and description, and the selected item's facts, description and steps. */
  class CardContent {
    constructor(markdown, goToStep) {
      this.markdown = markdown;
      this.goToStep = goToStep;
    }

    /** The overview's title, subtitle and description, or a step's title and description. */
    ofSlot(workflow, step) {
      const isOverview = step === null;
      const describedSlot = isOverview ? workflow : step;
      const title = createTextElement('h3', 'svugy-panel-title', describedSlot.title);
      const subtitle = isOverview ? createTextElement('p', 'svugy-panel-subtitle', workflow.subtitle) : null;
      const descriptionNodes = this.descriptionNodesOf(describedSlot.description);
      const headNodes = [title, subtitle].filter((node) => node !== null);
      return [...headNodes, ...descriptionNodes];
    }

    /** The selected item's facts and description by its kind, then one button per step it takes part in; nothing without an item. */
    ofItem(item, workflow, deepestLayer) {
      if (item === null) {
        return [];
      }
      const factNodes = item.factNodesBy(this, deepestLayer);
      const stepNodes = this.stepNodesOf(item.stepIds, workflow);
      return [...factNodes, ...stepNodes];
    }

    /** Label, caption, code, each metadata entry as `NAME: value`, description. */
    elementNodesOf(details) {
      const label = createTextElement('h3', 'svugy-panel-title', details.label ?? details.path);
      const caption = createTextElement('p', 'svugy-panel-subtitle', details.caption);
      const code = CardContent.codeNodeOf(details.code);
      const metadata = isPlainObject(details.metadata) ? details.metadata : {};
      const metadataEntries = Object.entries(metadata);
      const metadataNodes = metadataEntries.map(([name, value]) => {
        const valueText = typeof value === 'string' ? value : JSON.stringify(value);
        return createTextElement('p', 'svugy-panel-fact', `${name}: ${valueText}`);
      });
      const descriptionNodes = this.descriptionNodesOf(details.description);
      const nodes = [label, caption, code, ...metadataNodes, ...descriptionNodes];
      return nodes.filter((node) => node !== null);
    }

    /** A written edge's name and description, or a lifted connection's title then each source edge's name and description. */
    connectionNodesOf(item, deepestLayer) {
      const itemName = item.nameIn(deepestLayer);
      const title = createTextElement('h3', 'svugy-panel-title', itemName);
      if (item.isLifted) {
        const listedEdges = item.details.edges;
        const sourceEdges = Array.isArray(listedEdges) ? listedEdges.filter(isPlainObject) : [];
        const sourceEdgeSections = sourceEdges.map((sourceEdge) => this.sourceEdgeSectionOf(sourceEdge, deepestLayer));
        return [title, ...sourceEdgeSections];
      }
      const descriptionNodes = this.descriptionNodesOf(item.details.description);
      return [title, ...descriptionNodes];
    }

    sourceEdgeSectionOf(sourceEdge, deepestLayer) {
      const endpointsName = CardContent.endpointsNameOf(sourceEdge, deepestLayer);
      const headingText = sourceEdge.label ?? endpointsName;
      const heading = createTextElement('h4', 'svugy-panel-source-edge', headingText);
      const descriptionNodes = this.descriptionNodesOf(sourceEdge.description);
      const section = createXhtmlElement('section', {});
      section.append(heading, ...descriptionNodes);
      return section;
    }

    /** `Steps`, then one `i · TITLE` button per step of `workflow` among `stepIds`, in step order, going to that step. */
    stepNodesOf(stepIds, workflow) {
      const steps = workflow.steps.filter((step) => stepIds.includes(step.id));
      const isStepless = steps.length === 0;
      if (isStepless) {
        return [];
      }
      const heading = createTextElement('h4', 'svugy-panel-steps-heading', 'Steps');
      const stepButtons = steps.map((step) => {
        const stepNumber = workflow.steps.indexOf(step) + 1;
        const button = createXhtmlElement('button', { type: 'button', class: 'svugy-panel-step' });
        button.textContent = `${stepNumber} ${MIDDLE_DOT} ${step.title ?? step.id}`;
        button.addEventListener('click', () => this.goToStep(step.id));
        return button;
      });
      const stepList = createXhtmlElement('div', { class: 'svugy-panel-step-list' });
      stepList.append(...stepButtons);
      return [heading, stepList];
    }

    descriptionNodesOf(description) {
      const isDescribed = typeof description === 'string';
      return isDescribed ? this.markdown.blocksOf(description) : [];
    }

    static codeNodeOf(code) {
      const isCodeless = typeof code !== 'string';
      if (isCodeless) {
        return null;
      }
      const codeText = createXhtmlElement('code', {});
      codeText.textContent = code;
      const paragraph = createXhtmlElement('p', { class: 'svugy-panel-fact' });
      paragraph.appendChild(codeText);
      return paragraph;
    }

    static endpointsNameOf(details, deepestLayer) {
      const fromName = CardContent.nameOfElementAt(deepestLayer, details.from);
      const toName = CardContent.nameOfElementAt(deepestLayer, details.to);
      return `${fromName} ${ARROW} ${toName}`;
    }

    static nameOfElementAt(deepestLayer, path) {
      const element = Contract.elementAt(deepestLayer, path);
      const title = element === null ? null : Contract.titleOf(element);
      return title ?? path;
    }
  }

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
      const faces = Array.from(group.querySelectorAll(FACE_PATH_SELECTOR));
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

  /** Renders the panel's Markdown subset as XHTML nodes: paragraphs keeping their line breaks, `-`/`*` and `1.` lists nested one level, `#`–`###` headings as h4–h6, fenced code, `**bold**`, `*em*`, `` `code` ``, `[text](url)` links to http(s) or relative URLs, and `[text](#target)` buttons following a link to a diagram object or a step; author text only ever becomes text nodes, never markup. */
  class SafeMarkdown {
    constructor(resolveTarget, followTarget) {
      this.resolveTarget = resolveTarget;
      this.followTarget = followTarget;
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

    /** A button following the link to the element, edge or step `target` names, or the link's text alone when it names nothing. */
    targetLinkNodeOf(linkText, target) {
      const resolvedTarget = this.resolveTarget(target);
      if (resolvedTarget === null) {
        return this.elementHoldingLines('span', [linkText]);
      }
      const button = this.elementHoldingLines('button', [linkText]);
      button.setAttribute('type', 'button');
      button.setAttribute('class', 'svugy-panel-link');
      button.addEventListener('click', () => this.followTarget(resolvedTarget));
      const isStepTarget = resolvedTarget.group === undefined;
      if (isStepTarget) {
        return button;
      }
      const hoverColor = TargetColor.readableHoverColorOf(resolvedTarget.group);
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

  /** Translates pointer, focus and key events on the diagram into the reader's intents and the workflow menu. */
  const Interactions = {
    bind(svg, reader, workflowMenu) {
      Interactions.makeGroupsFocusable(svg);
      const clickMemory = { wasPreviousOnCanvas: false };
      svg.addEventListener('click', (event) => Interactions.followClick(event, reader, workflowMenu, clickMemory));
      svg.addEventListener('keydown', (event) => Interactions.followKey(event, reader, workflowMenu));
      svg.addEventListener('pointermove', (event) => {
        const group = Interactions.groupAt(event.target);
        reader.hover(group);
      });
      svg.addEventListener('pointerleave', () => reader.hover(null));
      svg.addEventListener('focusin', (event) => {
        const group = Interactions.groupAt(event.target);
        if (group !== null) {
          reader.hover(group);
        }
      });
      svg.addEventListener('focusout', () => reader.hover(null));
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
    },

    // a double click on empty canvas steps out once: its second click is ignored, but a canvas click right after a click on a group is a click of its own
    followClick(event, reader, workflowMenu, clickMemory) {
      const wasPreviousOnCanvas = clickMemory.wasPreviousOnCanvas;
      const heading = Interactions.headingAt(event.target);
      const group = Interactions.groupAt(event.target);
      const isOnCanvas = heading === null && group === null;
      clickMemory.wasPreviousOnCanvas = isOnCanvas;
      if (heading !== null) {
        workflowMenu?.toggle();
        return;
      }
      if (group !== null) {
        reader.activateGroup(group);
        return;
      }
      const isSecondClickOnCanvas = event.detail > 1 && wasPreviousOnCanvas;
      if (isSecondClickOnCanvas) {
        return;
      }
      reader.escape();
    },

    // Escape closes the open menu before undoing a step; ← and → change step while the panel is visible and the menu closed
    followKey(event, reader, workflowMenu) {
      const isMenuOpen = workflowMenu !== null && workflowMenu.isOpen;
      const key = event.key;
      if (key === 'Escape') {
        if (isMenuOpen) {
          workflowMenu.close();
          return;
        }
        reader.escape();
        return;
      }
      const slotDelta = SLOT_DELTA_BY_KEY.get(key);
      const isSlotKey = slotDelta !== undefined;
      if (isSlotKey) {
        const isPanelHidden = !reader.state.panel.isVisible;
        const isSlotKeyIgnored = isPanelHidden || isMenuOpen;
        if (isSlotKeyIgnored) {
          return;
        }
        event.preventDefault();
        reader.apply((state) => state.moveSlotBy(slotDelta));
        return;
      }
      const isActivationKey = key === 'Enter' || key === ' ';
      const isElementTarget = event.target instanceof Element;
      const isOtherKeyOrTarget = !isActivationKey || !isElementTarget;
      if (isOtherKeyOrTarget) {
        return;
      }
      const isFocusedHeading = event.target.matches(HEADING_SELECTOR);
      if (isFocusedHeading) {
        event.preventDefault();
        workflowMenu?.toggle();
        return;
      }
      const isFocusedGroup = event.target.matches(INTERACTIVE_SELECTOR);
      if (isFocusedGroup) {
        event.preventDefault();
        reader.activateGroup(event.target);
      }
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

  const SLOT_DELTA_BY_KEY = new Map([
    ['ArrowLeft', -1],
    ['ArrowRight', 1],
  ]);

  function distanceBetween(firstPoint, secondPoint) {
    return Math.hypot(secondPoint.x - firstPoint.x, secondPoint.y - firstPoint.y);
  }

  function centreOf(view) {
    return { x: view.x + view.width / 2, y: view.y + view.height / 2 };
  }

  function midpointOf(firstPoint, secondPoint) {
    return { x: (firstPoint.x + secondPoint.x) / 2, y: (firstPoint.y + secondPoint.y) / 2 };
  }

  function isPlainObject(value) {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
  }

  function showIf(node, isShown) {
    node.hidden = !isShown;
  }

  /** Creates an XHTML element holding `text` as its only text node; null when `text` is no string. */
  function createTextElement(name, className, text) {
    const isTextless = typeof text !== 'string';
    if (isTextless) {
      return null;
    }
    const element = createXhtmlElement(name, { class: className });
    element.textContent = text;
    return element;
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
