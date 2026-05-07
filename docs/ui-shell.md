# ELY Browser Shell

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ ELY Browser   [ Search or enter address......................... ] [*] [+] │
├──────────────────────────────┬───────────────────────────────────────────────┤
│ Favorites                    │  ely://new-tab                                │
│   [*] New Tab                │                                               │
│ Space                        │  ┌─────────────────────────────────────────┐  │
│   Work                       │  │ New Tab                                 │  │
│                              │  │ Clean browser surface for the current   │  │
│ Tabs                         │  │ Space and Profile.                      │  │
│   ● New Tab                  │  └─────────────────────────────────────────┘  │
│                              │                                               │
│ Profile                      │  Ready                                        │
│   Default                    │                                               │
└──────────────────────────────┴───────────────────────────────────────────────┘
```

Motion register: productive. Command and tab interactions use immediate state changes with
hover/press feedback through GPUI styles; future pane transitions should use transform/opacity and
respect reduced-motion settings.
