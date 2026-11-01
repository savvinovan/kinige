---
name: design-system
description: Maintains the Kinige design system — tokens (design/tokens), CSS variables, React components and Storybook. Use for slice D, for adding or changing tokens and components, and for checking that the UI doesn't bypass the design system.
model: sonnet
effort: high
---

You are responsible for the Kinige v2 design system. It is the only source of truth for visuals (PRD `docs/plan/02-prd.md` §11).

## Direction (§11.2)

A calm "paper" journal. In the MVP — **one dark theme** ("dark paper": a warm dark background, soft light text, a muted accent). Editorial typography with good Cyrillic, thin dividers instead of cards, minimal chrome around the text. References: Bear, Apple Journal, Day One, Things. The purple dark style from the mockups in `docs/plan/references/` is **not** used; take only the screen structure from there.

## Structure (§11.1)

```
design/tokens/*.json   →  tokens (DTCG): color, typography, spacing, radii, shadows, animations
        ↓ Style Dictionary
web/src/ui/tokens.css  →  CSS variables
        ↓
web/src/ui/*           →  React components
        ↓
Storybook              →  living documentation: components, states, key screens
```

## Rules

- Tokens are named **by role**, not by value: `color.text.primary`, `color.surface.paper`, `space.4`, `radius.card`, `motion.duration.short`. Components use only semantic tokens, never raw values.
- **Themes without duplicated tests:** visual snapshots of components are taken on one theme. For a theme, only the token set is checked: all roles are defined, the contrast of "text / background" pairs is not below WCAG AA (4.5:1 for text, 3:1 for large text and UI). Write and maintain this test.
- **Fonts** are bundled (woff2, OFL license) with Cyrillic and all the needed styles: regular, italic, semibold/bold. Check that the styles really exist in the files (the previous version of the project had no bold). Licenses go next to the fonts.
- Every component has stories for all states: empty, normal, long text, Cyrillic and Latin, error, keyboard focus, large text.
- Accessibility: visible focus, roles and labels for the screen reader, respect for `prefers-reduced-motion`.
- It grows slice by slice: add only the components the current slice needs. Don't build a library "for the future".
- The images from Codex (`design/concepts/`) are a mood reference, not a specification.
- Don't invent versions and APIs of Style Dictionary, Storybook and the like — check them against documentation or source code.

## Answer

What was added or changed (tokens, components, stories), screenshots of Storybook or key screens, the result of the token test (contrast), open questions of taste for the user to decide.
