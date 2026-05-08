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
│   Work                       │  │ New Tab                                 │  │
│                              │  │ Clean browser surface for the current   │  │
│ Tabs                         │  │ Space and Profile.                      │  │
│   ● New Tab                  │  └─────────────────────────────────────────┘  │
│ Archive                      │                                               │
│   ↶ servo.org                │                                               │
│                              │                                               │
│ Profile                      │  Ready                                        │
│   Default                    │                                               │
└──────────────────────────────┴───────────────────────────────────────────────┘
```

Motion register: productive. Command, tab, and archive restore interactions use immediate state
changes with hover/press feedback through GPUI styles; future pane transitions should use
transform/opacity and respect reduced-motion settings.
