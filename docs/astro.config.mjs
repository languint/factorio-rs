// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import react from "@astrojs/react";
import starlightSidebarTopics from "starlight-sidebar-topics";
import { coreSidebar } from "./src/sidebars/core.mjs";
import { guiSidebar } from "./src/sidebars/gui.mjs";

const site = "https://languint.github.io";
const base = "/factorio-rs";

// https://astro.build/config
export default defineConfig({
  site,
  base,
  integrations: [
    react(),
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
      components: {
        SocialIcons: "./src/components/SocialIcons.astro",
        PageTitle: "./src/components/PageTitle.astro",
        Footer: "./src/components/Footer.astro",
      },
      plugins: [
        starlightSidebarTopics(
          [
            {
              id: "core",
              label: "factorio-rs",
              link: "/intro/",
              icon: "open-book",
              items: coreSidebar,
            },
            {
              id: "playground",
              label: "Playground",
              link: "/playground/",
              icon: "puzzle",
              items: [{ label: "Playground", slug: "playground" }],
            },
            {
              id: "factorio-rs-gui",
              label: "factorio-rs-gui",
              link: "/ecosystem/factorio-rs-gui/",
              icon: "seti:react",
              badge: { text: "Ecosystem", variant: "tip" },
              items: guiSidebar,
            },
          ],
          {
            // Site splash lists all roots; it is not part of any topic sidebar.
            exclude: ["/", "/index"],
          },
        ),
      ],
    }),
  ],
});
