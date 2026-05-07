# GPUI Reference Audit

Local reference repositories:

| Reference | Local path | Product use |
|---|---|---|
| gpui-component | `references/gpui-component` | Root window, input, button, sidebar, dock, list patterns |
| awesome-gpui | `references/awesome-gpui` | Ecosystem index and repository mapping |
| adabraka-ui | `references/adabraka-ui` | Clean desktop component patterns |
| ferrum-flow | `references/ferrum-flow` | Future visual workflow panels |
| gpui-flow | `references/gpui-flow` | Future rule/workflow graph panels |
| gpui-form | `references/gpui-form` | Future settings form derive patterns |
| gpui-hooks | `references/gpui-hooks` | Future reusable component state patterns |
| gpui-nav | `references/gpui-nav` | Future settings/sidebar navigation |
| gpui-router | `references/gpui-router` | Future `ely://settings/*` routing |
| gpui-storybook | `references/gpui-storybook` | Future component story validation |
| gpui-symbols | `references/gpui-symbols` | Future platform symbol integration |
| gpui-tea | `references/gpui-tea` | Future event-loop state model |
| gpui-video-player | `references/gpui-video-player` | Future downloaded media preview |
| plotters-gpui | `references/plotters-gpui` | Future performance charts |
| sotf | `references/sotf` | `gpui-d3rs` and `gpui-px` references |

Copied patterns in the current implementation:

- `gpui-component::Root` as the first window view.
- `gpui_component::input::InputState` with typed `InputEvent` subscription.
- `gpui_component::button::Button` for command actions.
- GPUI model discipline: core state mutates once per user action, then the view calls `cx.notify()`.
