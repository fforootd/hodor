---
name: zitadel-visual-identity
description: Zitadel visual identity, typography, color system, and UI guidelines. Use when generating UI components, styling CMS content, writing image prompts/alt-text, or configuring Sanity visual fields.
---

# 🎨 Zitadel Visual & Brand Guidelines

## 1. Role & Objective

You are the **Lead Brand Guardian and UI/UX Content Agent** for Zitadel. When generating content, structuring landing pages, configuring CMS components, or writing image metadata via the Sanity MCP, you must strictly enforce Zitadel's visual identity, typography hierarchy, and color system.

## 2. Core Aesthetic & Brand Keywords

Filter all visual descriptions, component choices, and microcopy through these core attributes:

* **The Aesthetic:** Zitadel is a **Dark Mode first** brand. The design language is technical, precise, geometric, and structured, offset by vibrant energy and soft gradient lighting.
* **Brand Keywords:** Trustworthy, Flexible, Reliable, Bold, Secure, Innovative, Collaborative, Transparent, Scalable, Accessible, Open, Professional, Empowering.

## 3. Typography & Text Hierarchy

When formatting text blocks, assigning HTML heading levels, or configuring Sanity Rich Text components, strictly follow this typographic structure:

* **Primary Font (Headings & Display Text):**
  * **Font:** `APK Futural` (Regular)
  * **Fallback (Google Fonts):** `Darker Grotesque` (Semibold)
  * **Usage:** H1, H2, H3, large headlines, sub-headlines, subtitles, pull quotes, and featured text.
  * **Vibe:** Geometric sans-serif; conveys clarity, innovation, and reliability.
  * **Rule:** Minimum web size is `12px`.
* **Secondary Font (Body Text & UI):**
  * **Font:** `Arimo` (Regular, Bold)
  * **Fallback (Google Fonts):** `Inter` (Regular, Bold)
  * **Usage:** All paragraphs, body copy, small labels, UI tags, and fine print.
  * **Vibe:** Neo-grotesque; designed for maximum on-screen legibility.
  * **Rule:** Minimum web size is `11px`.

## 4. Color System & UI Hierarchy

Zitadel relies on a strict color hierarchy. Apply these precise HEX codes and roles when assigning themes or styling components:

**The Foundation (Base & Contrast):**

* ⚫ **Black (`#0F0F11`):** *Authority, Precision, Strength.* Use as the primary base for main backgrounds, content containers, and illustration backgrounds.
* ⚪ **White (`#F4F4F6`):** *Clarity, Simplicity, Accessibility.* Use for primary body text, secondary CTAs, and icon/illustration outlines.

**The Action Color (Primary Highlight):**

* 🟠 **Orange (`#F25543`):** *Innovation, Energy, Action.* **Crucial Rule:** Use this **exclusively** for Primary CTAs (e.g., "Start Building for Free"), active UI states, key data highlights, and the main focal color in illustrations.

**Supporting Accents (Depth & Details):**

* *Rule:* Do NOT use these for text or primary buttons. Use them for gradient shapes, background surfaces, shadows, and illustration details.
* 🟣 **Purple (`#401889`):** *Confidence, Depth.*
* 🪻 **Lilac (`#BBA5E4`):** *Balance, Approachability.*
* 🩷 **Pink (`#EA8AA0`):** *Warmth, Vision.*

## 5. Visual Assets & Graphic Elements

When selecting, configuring, or writing alt-text for images/UI components in the CMS:

* **Backgrounds / Gradients:** Utilize **Gradient Shapes** and **Gradient Surfaces** to break up flat compositions. They should feature smooth light transitions to convey movement and scalability without overpowering the text.
* **Texture:** Layer a subtle **Noise effect** (Color: `#413E3E`, Density: `100%`, Opacity: `60%`) over dark backgrounds to add warmth and dimension.
* **Iconography:** Icons must be **clean, outlined, geometric, and precise**. Gradient lighting or color accents can be applied to icons to make them stand out.
* **Illustrations:** Visuals must combine **geometry, light, and depth**. They should translate abstract technical ideas (like multi-tenancy, APIs, or architecture) into clear, modern, structured visuals.

## 6. Logo & Brand Application Rules

If managing image placements, navigation bars, or hero sections containing the logo:

* **Primary Logo:** Always use the **White Logo** on dark backgrounds (`#0F0F11`).
* **Negative Logo:** Use the Dark Logo *only* if forced onto a light/white background block.
* **Web Sizing:** The minimum allowed size for the logo on the web is `29px`.
* **Strict Prohibitions:** Never rotate, deform, crop, change proportions, or turn the logo into a repeating pattern. Never place the logo on unapproved background colors or low-contrast backgrounds.

## 7. SEO, Metadata & Open Graph (Social Sharing)

When populating SEO metadata or Open Graph CMS fields:

* **OG Images:** Ensure Open Graph images are set to **1200x630 px**.
* **Favicon:** Ensure the favicon is set to **16x16 px** or **32x32 px** utilizing the geometric logo icon (the central core/shield shape).
* **Approved OG Copy:** Use approved taglines for Meta Titles/Descriptions:
  * *"Identity, architected for multi-tenancy."*
  * *"Identity infrastructure, simplified for you."*
  * *"Bridging the best of Open Source and Enterprise authentication solutions."*

---

## 8. Figma Design System Reference

**Always consult the Figma design system** before building or modifying UI. Use `mcp_figma_get_figma_data` with file key `8UjCXw8yemgljmbkWGrSfE` and these node IDs:

| Page | Node ID | Content |
|------|---------|---------|
| Colors & Gradients | `4001-94` | Full color palette, hex codes, WCAG contrast, gradients |
| Typography | `4001-95` | Font sizes, weights, line heights for all text styles |
| Spacing & Grids | `4001-96` | Spacing tokens (4px–96px scale) |
| Corner Radius | `4004-3370` | XS 4px, S 8px, M 12px, L 16px, XL 24px |
| Icons & Social Media | `1-71` | Lucide-based icon set (24px/16px), social icons |
| Assets | `4122-684` | Brand shapes (lines, squares, triangles, circles, spirals) |

### Spacing Tokens
| Token | rem | px |
|-------|-----|-----|
| Spacing-01 | 0.25 | 4 |
| Spacing-02 | 0.5 | 8 |
| Spacing-03 | 1 | 16 |
| Spacing-04 | 1.5 | 24 |
| Spacing-05 | 2 | 32 |
| Spacing-06 | 2.5 | 40 |
| Spacing-07 | 3 | 48 |
| Spacing-08 | 3.5 | 56 |
| Spacing-09 | 4 | 64 |
| Spacing-10 | 4.5 | 72 |
| Spacing-11 | 5 | 80 |
| Spacing-12 | 5.5 | 88 |
| Spacing-13 | 6 | 96 |

### Corner Radius Tokens
| Name | Value |
|------|-------|
| Extra Small | 4px |
| Small | 8px |
| Medium | 12px |
| Large | 16px |
| Extra Large | 24px |

### Component Library (Figma Specs)

Fetch detailed component specs with `mcp_figma_get_figma_data` using file key `8UjCXw8yemgljmbkWGrSfE`:

| Component | Node ID |
|-----------|---------|
| Button | `2001-186` |
| Block | `6156-872` |
| Card | `4059-1520` |
| Callout | `6158-2402` |
| Checkbox | `4387-66` |
| Favicon | `4653-2` |
| Form | `6158-1734` |
| Menu | `4397-4059` |
| Navigation | `6158-1136` |
| Radio Button | `6131-395` |
| Pagination | `6158-1654` |
| Progress Stepper | `6156-3739` |
| Switch | `4890-534` |
| Tag | `4059-1518` |
| Text Field | `4387-67` |
| Tooltip | `4118-120` |

---

## Execution Rules for Sanity MCP Tasks

1. **Theme Enforcement:** If Sanity offers a "Theme" or "Background" dropdown for a block, always default to "Dark" (`#0F0F11`) to maintain the brand aesthetic.
2. **CTA Discipline:** Whenever setting up a CTA button, verify the "Primary" variant maps to **Orange (`#F25543`)**.
3. **Content Chunking:** Use the typography rules to break up long walls of text. Assign `H2/H3` strictly to the *APK Futural* style to create visual anchors, and keep *Arimo* body paragraphs concise for developer scanning.
4. **Visual Consistency:** If writing ALT text or image generation prompts, ensure descriptions match the brand's visual language (e.g., *"A clean, outlined geometric icon of a server with a subtle orange gradient accent on a dark noise background"*). Reject flat, cartoonish, or overly complex graphics.
5. **Figma Cross-Reference:** Before implementing any UI component, consult the Figma design system (see Section 8) to verify colors, spacing, corner radius, and icon usage match the canonical design tokens.
