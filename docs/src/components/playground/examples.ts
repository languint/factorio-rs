export type PlaygroundFileMap = Record<string, string>;

export type PlaygroundExample = {
  id: string;
  label: string;
  files: PlaygroundFileMap;
  activeFile?: string;
};

export const EXAMPLES: PlaygroundExample[] = [
  {
    id: "filtered-event",
    label: "Filtered event handler",
    files: {
      "control/on_built_entity.rs": `#[factorio_rs::event(filter = OnBuiltEntityFilter::name("inserter"))]
pub fn on_built_entity(event: OnBuiltEntityEvent) {
    let x = event.entity.position().x;
    let y = event.entity.position().y;
    println!("inserter built at: ({x},{y})");
}
`,
    },
  },
  {
    id: "remote-api",
    label: "Remote interface export",
    files: {
      "control/api.rs": `#[factorio_rs::export]
pub fn greet(name: &str) -> String {
    format!("hello, {name}")
}

#[factorio_rs::export(interface = "playground-admin")]
pub fn ping() {
    println!("pong");
}
`,
    },
  },
  {
    id: "storage",
    label: "Persist with storage",
    files: {
      "control/boot.rs": `#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let n = storage.get::<u32>("boots").unwrap_or(0);
    storage.set("boots", n + 1);
    println!("boot count: {}", n + 1);
}
`,
    },
  },
  {
    id: "enum-phase",
    label: "Enum state machine",
    activeFile: "control/tick.rs",
    files: {
      "shared/phase.rs": `pub enum Phase {
    Idle,
    Mining { ticks: i64 },
    Done,
}

impl Phase {
    pub fn tick(self) -> Phase {
        match self {
            Phase::Idle => Phase::Mining { ticks: 0 },
            Phase::Mining { ticks } if ticks + 1 >= 60 => Phase::Done,
            Phase::Mining { ticks } => Phase::Mining { ticks: ticks + 1 },
            Phase::Done => Phase::Done,
        }
    }
}
`,
      "control/tick.rs": `use crate::shared::phase::Phase;

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let mut phase = storage
        .get::<Phase>("phase")
        .unwrap_or(Phase::Idle);
    phase = phase.tick();
    storage.set("phase", phase);
    if matches!(phase, Phase::Mining { .. }) {
        println!("mining started");
    }
}
`,
    },
  },
  {
    id: "traits-dyn",
    label: "Traits and dyn dispatch",
    activeFile: "control/alerts.rs",
    files: {
      "shared/alert.rs": `pub trait Alert {
    fn title(&self) -> &'static str;
    fn priority(&self) -> i64;

    fn announce(&self) {
        println!("[alert p{}] {}", self.priority(), self.title());
    }
}
`,
      "control/alerts.rs": `use crate::shared::alert::Alert;

struct BeltJam {
    lane: &'static str,
}

impl Alert for BeltJam {
    fn title(&self) -> &'static str {
        self.lane
    }

    fn priority(&self) -> i64 {
        40
    }

    fn announce(&self) {
        println!("[belt jammed] {}", self.lane);
    }
}

fn shout(a: &dyn Alert) {
    a.announce();
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let jam = BeltJam {
        lane: "iron-plate",
    };
    jam.announce();
    shout(&jam);
}
`,
    },
  },
  {
    id: "result-try",
    label: "Result and ?",
    files: {
      "control/place.rs": `pub fn place(name: &str) -> Result<i32, &'static str> {
    let n = Some(1).ok_or("missing")?;
    if name.is_empty() {
        return Err("empty name");
    }
    Ok(n + 1)
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    match place("inserter") {
        Ok(n) => println!("ok count={n}"),
        Err(e) => println!("err: {e}"),
    }
}
`,
    },
  },
  {
    id: "prototypes",
    label: "Item, recipe, and locale",
    files: {
      "data/prototypes.rs": `item! {
    widget {
        name = "playground-widget",
        icon = "graphics/icon.png",
        stack_size = 50,
        icon_size = 64,
    }
}

recipe! {
    craft_widget {
        name = "playground-widget",
        energy_required = 1.0,
        ingredients = [
            { name = "iron-plate", amount = 2 },
            { name = "copper-plate", amount = 1 },
        ],
        results = [
            { name = Items::WIDGET, amount = 1 },
        ],
        category = "crafting",
        enabled = true,
    }
}

locale! {
    file = "names",
    en {
        "item-name" {
            Items::WIDGET = "Playground Widget",
        }
        "recipe-name" {
            Recipes::CRAFT_WIDGET = "Craft Widget",
        }
    }
}
`,
    },
  },
  {
    id: "iterators",
    label: "Iterator map / filter / collect",
    files: {
      "control/scores.rs": `pub fn top_half(scores: Vec<i64>) -> Vec<i64> {
    scores.iter().filter(|s| *s >= 50).collect::<Vec<_>>()
}

pub fn indices(n: i64) -> Vec<i64> {
    (0..n).map(|i| i + 1).collect::<Vec<_>>()
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let scores = (0..5).map(|i| i * 20).collect::<Vec<_>>();
    let xs = top_half(scores);
    let ys = indices(3);
    println!("kept={} first={}", xs.len(), ys[0]);
}
`,
    },
  },
  {
    id: "settings",
    label: "Mod settings",
    activeFile: "control/boot.rs",
    files: {
      "settings/mod.rs": `mod_settings! {
    prefix = "pg",
    startup {
        enabled: bool = true,
        speed: f64 = 1.0,
    }
    runtime_global {
        announce: bool = true,
    }
}

locale! {
    file = "settings",
    en {
        mod_setting_name {
            Settings::ENABLED = "Enable playground features",
            Settings::SPEED = "Simulation speed",
            Settings::ANNOUNCE = "Announce on tick",
        }
    }
}
`,
      "control/boot.rs": `use crate::settings::Settings;

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    if settings.startup.get_bool(Settings::ENABLED) {
        let speed = settings.startup.get::<f64>(Settings::SPEED);
        println!("playground enabled at speed={speed:?}");
    }
}
`,
    },
  },
  {
    id: "full-mod",
    label: "Full mini-mod",
    activeFile: "control/boot.rs",
    files: {
      "settings/mod.rs": `mod_settings! {
    prefix = "pg",
    startup {
        debug: bool = false,
    }
}
`,
      "data/prototypes.rs": `item! {
    widget {
        name = "playground-widget",
        icon = "graphics/icon.png",
        stack_size = 50,
        icon_size = 64,
    }
}

locale! {
    file = "item-names",
    en {
        "item-name" {
            Items::WIDGET = "Playground Widget",
        }
    }
}
`,
      "shared/phase.rs": `pub enum Phase {
    Idle,
    Running { ticks: i64 },
}

impl Phase {
    pub fn tick(self) -> Phase {
        match self {
            Phase::Idle => Phase::Running { ticks: 0 },
            Phase::Running { ticks } => Phase::Running { ticks: ticks + 1 },
        }
    }
}
`,
      "control/boot.rs": `use crate::shared::phase::Phase;

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    let mut phase = Phase::Idle;
    phase = phase.tick();
    let boots = storage.get::<u32>("boots").unwrap_or(0);
    storage.set("boots", boots + 1);
    storage.set("phase", phase);
    println!(
        "boot={} running={}",
        boots + 1,
        matches!(phase, Phase::Running { .. })
    );
}
`,
      "control/api.rs": `#[factorio_rs::export]
pub fn status() -> String {
    let boots = storage.get::<u32>("boots").unwrap_or(0);
    format!("boots={boots}")
}
`,
    },
  },
];

export function sortedPaths(files: PlaygroundFileMap): string[] {
  return Object.keys(files).sort((a, b) => a.localeCompare(b));
}

export function rustPathToLua(path: string): string {
  const withoutExt = path.replace(/\.rs$/u, "");
  const normalized = withoutExt.replace(/\/mod$/u, "");
  return `lua/${normalized}.lua`;
}

export function nextUntitledPath(
  files: PlaygroundFileMap,
  directory = "control",
): string {
  const prefix = directory.replace(/\/$/u, "");
  let index = 1;
  while (files[`${prefix}/file${index}.rs`]) {
    index += 1;
  }
  return `${prefix}/file${index}.rs`;
}

export function normalizeRustPath(path: string): string {
  return path.trim().replace(/^\//u, "").replace(/^src\//u, "");
}

export function parentDirectory(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? "" : path.slice(0, index);
}

export function pathsInFolder(paths: string[], folder: string): string[] {
  const prefix = `${folder}/`;
  return paths.filter((path) => path.startsWith(prefix));
}
