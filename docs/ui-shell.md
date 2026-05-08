# ELY Browser Shell

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ ELY Browser   [ Search or enter address.................... ] [pin] [*] [+] │
├──────────────────────────────┬───────────────────────────────────────────────┤
│ Favorites                    │  ely://new-tab                                │
│   [*] New Tab                │                                               │
│ Pinned                       │                                               │
│   [pin] New Tab              │                                               │
│ Space                        │  ┌─────────────────────────────────────────┐  │
│   W Work                     │  │ New Tab                                 │  │
│   R Research                 │  │ Clean browser surface for the current   │  │
│                              │  │ Space and Profile.                      │  │
│ Tabs                         │  └─────────────────────────────────────────┘  │
│   ● New Tab                  │                                               │
│ Archive                      │                                               │
│   ↶ servo.org                │                                               │
│     Space: Work - Profile: Default - Closed - Just now                      │
│                              │                                               │
│ Profile                      │  Ready                                        │
│   Default                    │                                               │
└──────────────────────────────┴───────────────────────────────────────────────┘
```

Motion register: productive. Command, space, tab, and archive restore interactions use immediate
state changes with hover/press feedback through GPUI styles; future pane transitions should use
transform/opacity and respect reduced-motion settings.
