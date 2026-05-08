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

Crash recovery uses the same productive register:

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ ELY Browser   [ https://example.com/crash-loop.............. ] [pin] [*] [+] │
├──────────────────────────────┬───────────────────────────────────────────────┤
│ Tabs                         │  ┌─────────────────────────────────────────┐  │
│   * example.com              │  │ ! Tab Recovery                [Restore] │  │
│     https://example.com/...  │  │ Recovering example.com                 │  │
│                              │  ├─────────────────────────────────────────┤  │
│ Profile                      │  │ URL        https://example.com/...      │  │
│   Default                    │  │ Title      example.com                  │  │
│                              │  │ Favicon    Saved: favicons/example.ico  │  │
│                              │  │ Space      Work                         │  │
│                              │  │ Profile    Default                      │  │
│                              │  │ Form restore prompt  Session data kept  │  │
│                              │  │ [Restore]                               │  │
│                              │  └─────────────────────────────────────────┘  │
└──────────────────────────────┴───────────────────────────────────────────────┘
```

Sleeping tabs keep the same layout rhythm:

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ ELY Browser   [ https://example.com/sleep................... ] [pin] [*] [+] │
├──────────────────────────────┬───────────────────────────────────────────────┤
│ Tabs                         │  ┌─────────────────────────────────────────┐  │
│   * example.com              │  │ Sleeping Tab                  [Restore] │  │
│     example.com              │  │ Sleeping example.com                   │  │
│                              │  ├─────────────────────────────────────────┤  │
│ Profile                      │  │ URL      https://example.com/sleep      │  │
│   Default                    │  │ Title    example.com                    │  │
│                              │  │ Favicon  No favicon saved              │  │
│                              │  │ Space    Work                           │  │
│                              │  │ Profile  Default                        │  │
│                              │  │ Session  Page session remains attached  │  │
│                              │  │ [Restore]                               │  │
│                              │  └─────────────────────────────────────────┘  │
└──────────────────────────────┴───────────────────────────────────────────────┘
```
