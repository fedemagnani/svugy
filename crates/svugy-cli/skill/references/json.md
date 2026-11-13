# svugy model JSON

The shape `svugy import FILE -o model.json` writes and `render`, `validate` and `import` read. It says what the Markdown says, with every REF resolved to a whole path.

```json
{
  "view": "top",
  "elements": {
    "web": { "label": "Web app" },
    "shop": { "label": "Shop backend" },
    "shop.api": { "label": "api", "caption": "REST" }
  },
  "workflows": [
    {
      "id": "checkout",
      "title": "A customer pays",
      "subtitle": "from cart to charged card",
      "description": "What happens between [the web app](#web) …",
      "overrides": {
        "shop": { "child_placement": "row" },
        "web": { "position": { "x": 0.0, "y": 40.0 } }
      },
      "edges": [
        { "from": "web", "to": "shop.api", "ordinal": 0, "label": "places order", "traveler": "envelope" }
      ],
      "steps": [
        {
          "id": "order",
          "title": "The shop takes the order",
          "description": "[The API](#shop.api) saves the cart …",
          "items": [
            { "edge": { "from": "web", "to": "shop.api", "ordinal": 0 } },
            { "element": "shop" }
          ]
        }
      ]
    }
  ]
}
```

- Root keys: `view` (optional), `elements`, `workflows` (≥ 1, in display order).
- `elements`: keyed by path; fields as v3, without `position` and `child_placement`.
- Workflow: `id`, `title`, `steps` (≥ 1) required; `subtitle`, `description`, `overrides`, `edges` optional.
- `overrides`: keyed by path; any of `caption`, `shape`, `height`, `style`, `child_placement`, `position`, `link`, `code`, `metadata`. `style` and `metadata` merge entry by entry over the element's.
- `edges`: v3 edge objects, `{from, to, ordinal}` unique within the workflow. Every edge is listed by at least one step.
- Step: `id`, `title`, `items` (≥ 1) required; `description` optional. An item is `{"edge": {from, to, ordinal}}`, naming an edge of the workflow, or `{"element": PATH}`.
- `span` fields are optional byte ranges into the source an importer read.
- Errors naming their fix: root `title`, `subtitle`, `description`, `legend` or `edges`; element `position` or `child_placement`.
