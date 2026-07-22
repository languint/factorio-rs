/**
 * Shared Starlight sidebar groups for the core factorio-rs docs topic.
 * Kept here so `astro.config.mjs` stays readable as more roots are added.
 */

/** @type {import('@astrojs/starlight/types').StarlightUserConfig['sidebar']} */
export const coreSidebar = [
  {
    label: "Start here",
    items: [
      { label: "Introduction", slug: "intro" },
      { label: "Installation", slug: "installation" },
      { label: "Getting started", slug: "guides/getting-started" },
      {
        label: "Changelog",
        link: "https://github.com/languint/factorio-rs/blob/main/CHANGELOG.md",
        attrs: { target: "_blank" },
      },
    ],
  },
  {
    label: "Recipes",
    items: [
      { label: "Overview", slug: "recipes" },
      { label: "First hour", slug: "recipes/first-hour" },
      {
        label: "Hot reload with Bacon",
        slug: "recipes/hot-reload-bacon",
      },
      { label: "Persist with storage", slug: "recipes/persist-storage" },
      {
        label: "Settings that change gameplay",
        slug: "recipes/settings-gameplay",
      },
      {
        label: "Filter entity lists",
        slug: "recipes/filter-entities",
      },
      {
        label: "State machines with enums",
        slug: "recipes/state-machines",
      },
      {
        label: "Package graphics",
        slug: "recipes/package-graphics",
      },
      {
        label: "GUI basics",
        slug: "recipes/gui-basics",
      },
      {
        label: "Reactive GUI",
        slug: "recipes/reactive-gui",
      },
      {
        label: "Share an API between mods",
        slug: "recipes/share-api",
      },
    ],
  },
  {
    label: "Language",
    items: [
      { label: "Supported Rust", slug: "guides/language" },
      { label: "Authoring macros", slug: "guides/authoring-macros" },
      { label: "Option and Result", slug: "guides/option-and-result" },
      { label: "Enums", slug: "language/enums" },
      {
        label: "Collections and iterators",
        slug: "language/collections",
      },
      { label: "Type aliases", slug: "language/type-aliases" },
    ],
  },
  {
    label: "Concepts",
    items: [
      { label: "Stages", slug: "guides/stages" },
      { label: "API types", slug: "guides/api-types" },
      { label: "Lints", slug: "guides/lints" },
      { label: "Profiles", slug: "guides/profiles" },
    ],
  },
  {
    label: "Modding",
    items: [
      { label: "Events and filters", slug: "guides/events" },
      { label: "Mod settings", slug: "guides/mod-settings" },
      { label: "Prototypes", slug: "guides/prototypes" },
      { label: "Locale", slug: "guides/locale" },
      {
        label: "Sharing code between mods",
        slug: "guides/dependencies",
      },
      { label: "Testing", slug: "guides/testing" },
    ],
  },
  {
    label: "Optional features",
    items: [
      { label: "Tracing", slug: "guides/tracing" },
      { label: "Serde / JSON", slug: "guides/serde" },
    ],
  },
  {
    label: "Reference",
    items: [
      { label: "CLI", slug: "reference/cli" },
      { label: "Factorio.toml", slug: "reference/factorio-toml" },
      { label: "Macros and attributes", slug: "reference/macros" },
    ],
  },
  {
    label: "Examples",
    items: [
      { label: "hello_world", slug: "examples/hello-world" },
      { label: "gui_basics", slug: "examples/gui-basics" },
      { label: "locale_test", slug: "examples/locale-test" },
      {
        label: "mandatory_spaghetti",
        slug: "examples/mandatory-spaghetti",
      },
      { label: "tracing_test", slug: "examples/tracing-test" },
      { label: "traits_demo", slug: "examples/traits-demo" },
      { label: "provider / consumer", slug: "examples/dependencies" },
    ],
  },
];
