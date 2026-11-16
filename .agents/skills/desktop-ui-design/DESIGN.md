---
version: alpha
name: Frosted Glass Design System
colors:
  ink: "#202437"
  muted: "#878CA1"
  canvas-base: "#EDF2FD"
  blue: "#5688EA"
  blue-deep: "#3768C6"
  mint: "#68C7AE"
  coral: "#EE937E"
  glass: "rgba(255, 255, 255, 0.63)"
  glass-strong: "rgba(255, 255, 255, 0.78)"
  line: "rgba(115, 121, 153, 0.13)"
  white: "#FFFFFF"
typography:
  display:
    fontFamily: "Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI, PingFang SC, Microsoft YaHei, sans-serif"
    fontSize: 31px
    fontWeight: 690
    lineHeight: 1.2
    letterSpacing: -0.045em
  heading:
    fontFamily: "Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI, PingFang SC, Microsoft YaHei, sans-serif"
    fontSize: 16px
    fontWeight: 690
    lineHeight: 1.4
  body:
    fontFamily: "Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI, PingFang SC, Microsoft YaHei, sans-serif"
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.6
  caption:
    fontFamily: "Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI, PingFang SC, Microsoft YaHei, sans-serif"
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, Segoe UI, PingFang SC, Microsoft YaHei, sans-serif"
    fontSize: 13px
    fontWeight: 650
    lineHeight: 1.4
rounded:
  sm: 7px
  md: 9px
  lg: 14px
  xl: 16px
  window: 0px
spacing:
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  2xl: 32px
  3xl: 40px
components:
  app-window:
    backgroundColor: "rgba(248, 249, 255, 0.29)"
    textColor: "{colors.ink}"
    rounded: "{rounded.window}"
    padding: "0px"
  glass-card:
    backgroundColor: "{colors.glass}"
    textColor: "{colors.ink}"
    rounded: "{rounded.lg}"
    padding: "{spacing.lg}"
  active-navigation:
    backgroundColor: "{colors.glass-strong}"
    textColor: "{colors.blue-deep}"
    rounded: "{rounded.md}"
    padding: "7px 11px"
  primary-action:
    backgroundColor: "{colors.blue}"
    textColor: "{colors.white}"
    rounded: "{rounded.sm}"
    padding: "5px 10px"
---

## Overview

This is a reusable visual system for desktop interfaces. It should feel calm, clear, compact, and quietly warm. Use a light blue foundation, translucent white surfaces, and restrained blue accents. The desktop shell has a persistent left navigation and a broad content area. Favor compact desktop controls and list spacing while preserving readable text and clear content groups. Do not imply a specific brand, organization, or product.

The visual style is light glassmorphism. Frosted surfaces sit over a softly colored background; blur and transparency should add depth without reducing text clarity. Keep the Chinese interface friendly and direct. Use short labels, quiet supporting text, and clear hierarchy rather than decorative copy.

## Colors

The canvas uses layered blue, mint, and warm coral radial gradients over `canvas-base`. Glass surfaces are translucent white. Use `ink` for primary text and `muted` for secondary labels. Blue is the main interaction color; mint signals positive progress and coral is reserved for warm status accents.

- **Ink (`{colors.ink}`):** Main headings, values, and high-priority labels.
- **Muted (`{colors.muted}`):** Supporting copy and metadata. Preserve readable contrast on glass surfaces.
- **Blue (`{colors.blue}`):** Primary accent for active controls, chart data, and progress.
- **Blue deep (`{colors.blue-deep}`):** Blue text on pale backgrounds.
- **Mint (`{colors.mint}`):** Positive progress and completed-work accents.
- **Coral (`{colors.coral}`):** Warm secondary accent for time and review states.
- **Glass (`{colors.glass}`) and glass strong (`{colors.glass-strong}`):** Translucent surfaces; use the stronger value for selected or foreground controls.
- **Line (`{colors.line}`):** Quiet separators and surface outlines.

Keep gradients soft and broad. Do not use blue, mint, or coral as large saturated page backgrounds. Status meaning should also be conveyed with text or icons, not color alone.

## Typography

Use the system UI stack shown in the tokens, with Inter and native Chinese sans-serif fallbacks. Use readable desktop typography: a 28–31px greeting, 16px section headings, 14px body copy and controls, and at least 13px metadata. Never shrink text below 13px, including at responsive breakpoints. Use medium or semibold weights for hierarchy; reserve heavier weights for key values. Keep headings slightly tight, but do not apply display tracking to body copy.

On narrow screens, scale the greeting down while keeping body and control labels comfortable to read. Do not shrink essential text just to preserve desktop density.

## Layout

Fill the entire viewport with the application shell, with no outer margin, border, radius, or maximum width. At desktop widths, use a 200px sidebar and a flexible main column, with about 22px page padding. Keep the sidebar and each page header stationary. Place the page title, description, and primary action above an independently scrolling content area; the document itself must not scroll. Use a 4px spacing rhythm, with 6–12px gaps inside groups and 14–24px between major sections. Task rows use 10–12px vertical padding and 6–8px separation. Preserve body and metadata font sizes when increasing density.

For overview-style screens, arrange summary metrics in four equal columns. Pair a chart with a summary card, then place a dense list beside supporting content. Keep item names and status visible at a glance. Treat these as adaptable layout patterns, not required product sections. Below 1100px, reduce the sidebar width and use two metric columns. Below 760px, turn the sidebar into a stationary top navigation and stack the content panels. Preserve readable spacing and avoid horizontal scrolling.

## Elevation & Depth

Build depth from layered translucency rather than heavy borders. Apply a 18–28px backdrop blur to the application shell and cards. Use a thin white highlight at card edges, with a soft cool shadow beneath cards. Keep separators low contrast. Modals may use a stronger shadow and an 8px blurred scrim so they read as the top layer.

Avoid stacking many shadows on small controls. The chart area and status chips should remain visually light and close to their parent card.

## Shapes

Keep the outer desktop window square and flush with the viewport. Use a 12–16px radius for content cards, and 6–9px for buttons, inputs, badges, and navigation rows. Avatars and icon tiles can use compact rounded squares; reserve circles for avatars and chart points. Keep outlines thin and consistent.

## Components

- **Application shell:** Full-window translucent container over the gradient canvas. Separate the sidebar from content with a subtle vertical line.
- **Sidebar navigation:** Quiet text and line icons by default. The selected item gets a stronger glass fill, blue text, and a narrow blue edge marker.
- **Metric card:** Label and tinted icon tile at the top, prominent value below, and a short trend or context line at the bottom.
- **Content card:** White glass surface with a compact title, muted subtitle, and consistent inner padding. Align neighboring card headings on the same baseline.
- **Progress bar:** Thin rounded track with a blue, mint, or coral fill according to the item context. Include a visible numeric percentage.
- **Dense list row:** Optional leading mark and title on the left, compact metadata and progress in the middle, and a text status chip on the right. Allow the row to simplify on small screens.
- **Primary action:** Use a blue treatment with white text. Secondary actions should be transparent or lightly outlined. Ordinary buttons are about 33px tall, with a 14px label and 5px vertical padding; page primary actions may use 6px vertical padding.
- **Search and form fields:** Use a translucent white fill, fine outline, compact rounded corners, and a clear focus ring. Fields are about 35px tall with 6px vertical padding. Keep labels visible when a field has a value.
- **Modal:** Centered pale glass panel over a blurred dim scrim fixed to the viewport. Mount overlays outside the application shell so ancestor blur cannot change their containing block. Lock background scrolling while open and allow tall dialogs to scroll internally. Keep its title, fields, and primary action in a clear vertical sequence.

Interactive controls need visible hover, focus, and active states. Maintain text contrast on translucent backgrounds; do not communicate selection or status using color alone.

## Do's and Don'ts

- Do keep the overall canvas pale, soft, and low-noise.
- Do use blue consistently for primary actions, selected navigation, and the main chart series.
- Do use mint for positive progress and completed work, and coral for occasional warm accents.
- Do preserve a clear hierarchy between navigation, key information, primary content, and supporting details.
- Do use blur sparingly enough that text and control edges stay crisp.
- Do keep labels concise and supporting copy visually secondary.
- Don't use dark glass, neon accents, or large saturated gradients.
- Don't make every card equally prominent; reserve stronger contrast for the active task and key metrics.
- Don't rely on tiny pale text for important instructions or status.
- Don't add decorative imagery that competes with primary data or content.
