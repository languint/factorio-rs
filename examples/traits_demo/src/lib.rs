pub mod shared;

#[factorio_rs::control]
mod control {
    use crate::shared::alert::Alert;

    struct PowerDrop {
        machine: &'static str,
        percent: i64,
    }

    impl Alert for PowerDrop {
        fn title(&self) -> &'static str {
            self.machine
        }

        fn priority(&self) -> i64 {
            100 - self.percent
        }
    }

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

    struct ScienceStall {
        pack: &'static str,
    }

    impl Alert for ScienceStall {
        fn title(&self) -> &'static str {
            self.pack
        }

        fn priority(&self) -> i64 {
            90
        }
    }

    fn shout(alert: &dyn Alert) {
        alert.announce();
    }

    fn priority_of(alert: &dyn Alert) -> i64 {
        alert.priority()
    }

    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        let power = PowerDrop {
            machine: "assembling-machine-2",
            percent: 15,
        };
        let belt = BeltJam {
            lane: "iron-plate main",
        };
        let science = ScienceStall {
            pack: "chemical-science-pack",
        };

        power.announce();
        belt.announce();
        science.announce();

        let total = priority_of(&power as &dyn Alert)
            + priority_of(&belt as &dyn Alert)
            + priority_of(&science as &dyn Alert);

        shout(&PowerDrop {
            machine: "electric-furnace",
            percent: 5,
        } as &dyn Alert);
        shout(&BeltJam {
            lane: "copper green",
        } as &dyn Alert);
        shout(&ScienceStall {
            pack: "production-science-pack",
        } as &dyn Alert);

        println!("total alert priority: {total}");
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn static_priorities() {
            let power = PowerDrop {
                machine: "lab",
                percent: 20,
            };
            assert_eq!(power.priority(), 80);
            assert_eq!(BeltJam { lane: "main" }.priority(), 40);
            assert_eq!(
                ScienceStall {
                    pack: "automation-science-pack",
                }
                .priority(),
                90
            );
        }
        #[test]
        fn dyn_dispatch_sums_priorities() {
            let total = priority_of(&PowerDrop {
                machine: "lab",
                percent: 20,
            } as &dyn Alert)
                + priority_of(&BeltJam { lane: "main" } as &dyn Alert)
                + priority_of(&ScienceStall {
                    pack: "automation-science-pack",
                } as &dyn Alert);
            assert_eq!(total, 210);
        }
    }
}
