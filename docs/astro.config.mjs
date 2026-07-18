// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

const site = "https://languint.github.io";
const base = "/factorio-rs";

// https://astro.build/config
export default defineConfig({
  site,
  base,
  integrations: [
    starlight({
      title: "factorio-rs",
      description:
        "Write Factorio mods in Rust. factorio-rs transpiles Rust to loadable Lua mods with typed API bindings, events, settings, and locale.",
      logo: {
        src: "./src/assets/logo.svg",
      },
      customCss: ["./src/styles/custom.css"],
      head: [
        {
          tag: "meta",
          attrs: {
            name: "keywords",
            content:
              "factorio, factorio mods, rust, lua, transpile, modding, gamedev, factorio-rs",
          },
        },
      ],
      routeMiddleware: "./src/routeData.ts",
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/languint/factorio-rs",
        },
      ],
      editLink: {
        baseUrl: "https://github.com/languint/factorio-rs/edit/main/docs/",
      },
      sidebar: [
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
          label: "Guides",
          items: [
            { label: "Stages", slug: "guides/stages" },
            { label: "Language support", slug: "guides/language" },
            { label: "Option and Result", slug: "guides/option-and-result" },
            { label: "Sharing code between mods", slug: "guides/dependencies" },
            { label: "API types", slug: "guides/api-types" },
            { label: "Events and filters", slug: "guides/events" },
            { label: "Mod settings", slug: "guides/mod-settings" },
            { label: "Locale", slug: "guides/locale" },
            { label: "Lints", slug: "guides/lints" },
            { label: "Profiles", slug: "guides/profiles" },
          ],
        },
        {
          label: "Features",
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
            { label: "locale_test", slug: "examples/locale-test" },
            {
              label: "mandatory_spaghetti",
              slug: "examples/mandatory-spaghetti",
            },
            { label: "tracing_test", slug: "examples/tracing-test" },
            { label: "provider / consumer", slug: "examples/dependencies" },
          ],
        },
      ],
    }),
  ],
});
