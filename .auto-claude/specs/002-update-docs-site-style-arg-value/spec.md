# Quick Spec: Red/Black Color Scheme for Docs Site

## Overview

This spec updates the documentation site's visual styling to implement a red and black color scheme. The change will be applied by overriding CSS variables in the global stylesheet, leveraging Fumadocs UI's theming system with Tailwind CSS v4.

## Workflow Type

Feature

## Task Scope
Update the documentation site styling to use a red and black color scheme.

## Files to Modify
- `docs/src/app/global.css` - Add CSS variable overrides for red/black theme

## Change Details

The docs site uses Fumadocs UI with Tailwind CSS v4, which uses CSS variables for theming. We need to override the primary color variables to use red instead of the default blue.

Add CSS variable overrides after the existing imports to set:
- Primary color: Red (#DC2626 or similar)
- Background: Black/dark (#0A0A0A or similar)
- Text: White/light for contrast on dark backgrounds

Example variables to override:
```css
:root {
  --fd-primary: #DC2626;
  --fd-primary-foreground: #FFFFFF;
  --fd-background: #0A0A0A;
  --fd-foreground: #F5F5F5;
}
```

## Verification
- [ ] Docs site displays with red accents
- [ ] Background is dark/black
- [ ] Text is readable with good contrast
- [ ] Links and buttons appear red
- [ ] Run `cd docs && bun run dev` to visually verify

## Success Criteria

- [ ] Documentation site displays with red accent color (#DC2626)
- [ ] Background color is dark/black (#0A0A0A)
- [ ] Text color provides adequate contrast (WCAG AA compliant)
- [ ] All UI components (buttons, links, badges) reflect the new color scheme
- [ ] No visual artifacts or styling inconsistencies
- [ ] Dev server runs without errors

## Notes
- Fumadocs UI uses CSS variables prefixed with `--fd-` for theming
- Tailwind v4 uses CSS variables for configuration
- No separate tailwind.config file needed (v4 uses CSS-based config)
- Test color contrast for accessibility
