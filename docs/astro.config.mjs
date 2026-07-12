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
        "Rust SDK for Factorio modding - transpile Rust to Lua mods.",
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
          ],
        },
        {
          label: "Guides",
          items: [
            { label: "Stages", slug: "guides/stages" },
            { label: "Language support", slug: "guides/language" },
            { label: "API types", slug: "guides/api-types" },
            { label: "Events and filters", slug: "guides/events" },
            { label: "Mod settings", slug: "guides/mod-settings" },
            { label: "Locale", slug: "guides/locale" },
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
            {
              label: "mandatory_spaghetti",
              slug: "examples/mandatory-spaghetti",
            },
            { label: "tracing_test", slug: "examples/tracing-test" },
          ],
        },
      ],
    }),
  ],
});
