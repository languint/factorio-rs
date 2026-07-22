/** @type {import('@astrojs/starlight/types').StarlightUserConfig['sidebar']} */
export const guiSidebar = [
  { label: "Overview", slug: "ecosystem/factorio-rs-gui" },
  {
    label: "Guides",
    items: [
      { label: "Guides overview", slug: "ecosystem/factorio-rs-gui/guides" },
      {
        label: "Getting started",
        slug: "ecosystem/factorio-rs-gui/guides/getting-started",
      },
      { label: "State", slug: "ecosystem/factorio-rs-gui/guides/state" },
      { label: "Lifecycle", slug: "ecosystem/factorio-rs-gui/guides/lifecycle" },
      { label: "Reactive GUI", slug: "ecosystem/factorio-rs-gui/guides/reactive" },
      {
        label: "Multiple windows",
        slug: "ecosystem/factorio-rs-gui/guides/multiple-windows",
      },
    ],
  },
  {
    label: "Widgets",
    items: [
      { label: "Widgets overview", slug: "ecosystem/factorio-rs-gui/widgets" },
      { label: "Widget", slug: "ecosystem/factorio-rs-gui/widgets/widget" },
      { label: "Frame", slug: "ecosystem/factorio-rs-gui/widgets/frame" },
      { label: "Text", slug: "ecosystem/factorio-rs-gui/widgets/text" },
      { label: "Button", slug: "ecosystem/factorio-rs-gui/widgets/button" },
    ],
  },
];
