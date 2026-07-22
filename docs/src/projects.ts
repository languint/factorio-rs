export type DocProjectKind = "core" | "tool" | "ecosystem";

export type DocProject = {
  id: string;
  title: string;
  description: string;
  href: string;
  kind: DocProjectKind;
  /** Optional badge text shown on the index card. */
  badge?: string;
};

export const PROJECTS: readonly DocProject[] = [
  {
    id: "core",
    title: "factorio-rs",
    description:
      "Language, CLI, recipes, and reference for writing Factorio mods in Rust.",
    href: "intro/",
    kind: "core",
  },
  {
    id: "playground",
    title: "Playground",
    description:
      "Live browser transpile preview: Rust in, Factorio mod tree out.",
    href: "playground/",
    kind: "tool",
  },
  {
    id: "factorio-rs-gui",
    title: "factorio-rs-gui",
    description:
      "Reactive GUI helpers (crates.io + Factorio mod portal library mod).",
    href: "ecosystem/factorio-rs-gui/",
    kind: "ecosystem",
    badge: "Ecosystem",
  },
] as const;

export function projectById(id: string): DocProject | undefined {
  return PROJECTS.find((project) => project.id === id);
}

/** True when the Starlight entry id is the playground page. */
export function isPlaygroundEntryId(id: string): boolean {
  return id === "playground" || id.endsWith("/playground");
}
