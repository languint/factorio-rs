import { defineRouteMiddleware } from "@astrojs/starlight/route-data";

const DOCS_ORIGIN = "https://languint.github.io";
const DOCS_BASE = "/factorio-rs";
const OG_IMAGE = `${DOCS_ORIGIN}${DOCS_BASE}/og/social-v2.png`;
const GITHUB_URL = "https://github.com/languint/factorio-rs";

export const onRequest = defineRouteMiddleware((context) => {
  const { entry, head, siteTitle } = context.locals.starlightRoute;
  const title = entry.data.title;
  const description =
    entry.data.description ??
    "Rust SDK for Factorio modding - transpile Rust to Lua mods.";
  const pageUrl = new URL(context.url.pathname, context.site).href;

  const isHome =
    entry.id === "index" ||
    entry.id === "" ||
    context.url.pathname.replace(/\/$/, "") === DOCS_BASE;

  // Avoid "factorio-rs | factorio-rs" on the splash page.
  if (isHome) {
    const titleTag = head.find((tag) => tag.tag === "title");
    if (titleTag) {
      titleTag.content = `${siteTitle} | Write Factorio mods in Rust`;
    }
    const ogTitle = head.find(
      (tag) => tag.tag === "meta" && tag.attrs?.property === "og:title",
    );
    if (ogTitle?.attrs) {
      ogTitle.attrs.content = `${siteTitle} | Write Factorio mods in Rust`;
    }
  }

  head.push({
    tag: "meta",
    attrs: { property: "og:image", content: OG_IMAGE },
  });
  head.push({
    tag: "meta",
    attrs: { property: "og:image:alt", content: `${siteTitle} documentation` },
  });
  head.push({
    tag: "meta",
    attrs: { property: "og:image:width", content: "1200" },
  });
  head.push({
    tag: "meta",
    attrs: { property: "og:image:height", content: "630" },
  });
  head.push({
    tag: "meta",
    attrs: { name: "twitter:image", content: OG_IMAGE },
  });
  head.push({
    tag: "meta",
    attrs: {
      name: "twitter:title",
      content: isHome ? `${siteTitle} | Write Factorio mods in Rust` : title,
    },
  });
  head.push({
    tag: "meta",
    attrs: { name: "twitter:description", content: description },
  });

  const jsonLd = isHome
    ? {
        "@context": "https://schema.org",
        "@graph": [
          {
            "@type": "WebSite",
            name: siteTitle,
            url: `${DOCS_ORIGIN}${DOCS_BASE}/`,
            description,
            publisher: {
              "@type": "Organization",
              name: "factorio-rs",
              url: GITHUB_URL,
            },
          },
          {
            "@type": "SoftwareSourceCode",
            name: "factorio-rs",
            description:
              "Rust SDK for Factorio modding that transpiles Rust to loadable Lua mods.",
            url: `${DOCS_ORIGIN}${DOCS_BASE}/`,
            codeRepository: GITHUB_URL,
            programmingLanguage: ["Rust", "Lua"],
            runtimePlatform: "Factorio",
            license: "https://opensource.org/licenses/MIT",
          },
        ],
      }
    : {
        "@context": "https://schema.org",
        "@type": "TechArticle",
        headline: title,
        description,
        url: pageUrl,
        isPartOf: {
          "@type": "WebSite",
          name: siteTitle,
          url: `${DOCS_ORIGIN}${DOCS_BASE}/`,
        },
        author: {
          "@type": "Organization",
          name: "factorio-rs",
          url: GITHUB_URL,
        },
      };

  head.push({
    tag: "script",
    attrs: { type: "application/ld+json" },
    content: JSON.stringify(jsonLd),
  });
});
