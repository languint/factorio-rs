factorio_rs::control_mod! {
    use factorio_rs::{factorio_api::IndexOrName, prelude::*};

    /// Locale keys for greetings (`__1__` is the player name).
    /// Indices are 1-based in the `/greet <n>` command (`/greet 1` ... `/greet 3`).
    const GREETINGS: &[&str] = &[
        "greetings.hello",
        "greetings.welcome",
        "greetings.howdy",
    ];

    factorio_rs::locale! {
        en {
            greetings {
                "hello" = "Hello, __1__!",
                "welcome" = "Welcome, __1__!",
                "howdy" = "Howdy, __1__!",
                "usage" = "Usage: /greet <1-3>",
                "command-help" = "Print a localized greeting (/greet <1-3>)",
            }
        }

        de {
            greetings {
                "hello" = "Hallo, __1__!",
                "welcome" = "Willkommen, __1__!",
                "howdy" = "Servus, __1__!",
                "usage" = "Verwendung: /greet <1-3>",
                "command-help" = "Gibt eine lokalisierte Begrüßung aus (/greet <1-3>)",
            }
        }

        es {
            greetings {
                "hello" = "¡Hola, __1__!",
                "welcome" = "¡Bienvenido, __1__!",
                "howdy" = "¡Qué tal, __1__!",
                "usage" = "Uso: /greet <1-3>",
                "command-help" = "Muestra un saludo localizado (/greet <1-3>)",
            }
        }
    }

    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        commands.add_command("greet", ["greetings.command-help"], lua_fn(greet));
    }

    pub fn greet(command: CustomCommandData) {
        if let Some(player_index) = command.player_index
            && let Some(player) = game.get_player(IndexOrName::Index(player_index))
        {
            if let Some(parameter) = command.parameter {
                if parameter == "1" {
                    player.print([GREETINGS[0], player.name()], None);
                } else if parameter == "2" {
                    player.print([GREETINGS[1], player.name()], None);
                } else if parameter == "3" {
                    player.print([GREETINGS[2], player.name()], None);
                } else {
                    player.print(["greetings.usage"], None);
                }
            } else {
                player.print(["greetings.usage"], None);
            }
        }
    }
}
