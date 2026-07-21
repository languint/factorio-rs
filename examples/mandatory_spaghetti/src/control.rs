use factorio_rs::{
    factorio_api::{
        IndexOrName,
        classes::{
            self, LuaEntityDestroyParams, LuaEntityMineParams, LuaRenderingDrawLineParams,
            LuaSurfaceSpillInventoryParams,
        },
        concepts::{
            BoundingBox, Color, EntityID, EntitySearchFilters, EntityWithQualityID, ForceID,
            ForceSet, MapPosition, ScriptRenderTarget, SurfaceIdentification,
        },
    },
    prelude::*,
};

use crate::{adjacent_blacklist, pattern_blacklist, settings::Settings};

const CASUAL_MODE: bool = settings.startup.get_bool(Settings::CASUAL_MODE);
const ADJACENCY_ENABLED: bool = settings.startup.get_bool(Settings::ADJACENCY_ENABLED);

fn die(source: classes::LuaEntity) {
    let surface = source.surface();
    let position = source.position();
    let force = source.force();

    if CASUAL_MODE {
        let mut size: u16 = 0;
        if let Some(products) = source.prototype().mineable_properties().products {
            size = products.len() as u16;
        }
        let inventory = game.create_inventory(size, None);
        source.mine(LuaEntityMineParams {
            force: Some(true),
            inventory: Some(inventory),
            ..Default::default()
        });
        surface.spill_inventory(LuaSurfaceSpillInventoryParams {
            allow_belts: Some(false),
            enable_looted: Some(true),
            inventory,
            position,
            force: Some(ForceID::Force(force)),
            ..Default::default()
        });
    } else {
        source.die(None, None);
    }

    if let Some(ghost) = surface.find_entity(EntityWithQualityID::Name("entity-ghost"), position) {
        ghost.destroy(LuaEntityDestroyParams::default());
    }
}

fn adjacency(source: classes::LuaEntity, player_index: u32) {
    if adjacent_blacklist::check(source.r#type()) {
        return;
    }

    if player_index != 0
        && let Some(player) = game.get_player(IndexOrName::Index(player_index))
    {
        let cursor = player.cursor_stack();
        if cursor.valid_for_read() && cursor.r#type() == "rail-planner" {
            return;
        }
    }

    let surface = source.surface();
    let bb = source.bounding_box();
    let area = BoundingBox {
        left_top: MapPosition {
            x: bb.left_top.x - 1.0,
            y: bb.left_top.y - 1.0,
        },
        right_bottom: MapPosition {
            x: bb.right_bottom.x + 1.0,
            y: bb.right_bottom.y + 1.0,
        },
        ..Default::default()
    };
    let entities = surface.find_entities_filtered(EntitySearchFilters {
        area: Some(area),
        force: Some(ForceSet::One(ForceID::Force(source.force()))),
        ..Default::default()
    });

    let mut adjacent_count: u32 = 0;
    for entity in entities {
        if entity == source {
            continue;
        }
        if entity.prototype().is_building() {
            adjacent_count += 1;
        }
    }

    if adjacent_count > 1 {
        die(source);
    }
}

fn find_pattern(source: classes::LuaEntity, mut offset: MapPosition) -> Vec<classes::LuaEntity> {
    let mut pos = source.position();

    #[allow(clippy::manual_swap)]
    if pos.x > offset.x {
        let tmp = pos.x;
        pos.x = offset.x;
        offset.x = tmp;
    }
    #[allow(clippy::manual_swap)]
    if pos.y > offset.y {
        let tmp = pos.y;
        pos.y = offset.y;
        offset.y = tmp;
    }

    let bb = source.prototype().collision_box();
    let is_rectangular = bb.left_top.x != bb.left_top.y
        || bb.right_bottom.x != bb.right_bottom.y
        || bb.left_top.x != (0.0 - bb.right_bottom.x);

    let direction = if is_rectangular {
        Some(source.direction())
    } else {
        None
    };

    let entities = source
        .surface()
        .find_entities_filtered(EntitySearchFilters {
            area: Some(BoundingBox {
                left_top: pos,
                right_bottom: offset,
                ..Default::default()
            }),
            name: Some(EntityID::Name(source.name())),
            direction,
            force: Some(ForceSet::One(ForceID::Force(source.force()))),
            ..Default::default()
        });

    pos = source.position();

    let mut result: Vec<classes::LuaEntity> = Vec::new();
    for entity in entities {
        if entity == source {
            continue;
        }
        let pos2 = entity.position();
        if pos2.x == pos.x || pos2.y == pos.y {
            result.push(entity);
        }
    }
    result
}

fn draw_line(
    surface: classes::LuaSurface,
    from: ScriptRenderTarget,
    to: ScriptRenderTarget,
) -> classes::LuaRenderObject {
    rendering.draw_line(LuaRenderingDrawLineParams {
        width: 4.0,
        color: Color {
            r: Some(1.0),
            g: Some(1.0),
            b: Some(1.0),
            a: Some(1.0),
        },
        from,
        to,
        surface: SurfaceIdentification::Surface(surface),
        dash_length: Some(0.5),
        gap_length: Some(0.5),
        time_to_live: Some(60),
        dash_offset: Some(0.25),
        blink_interval: Some(15),
        ..Default::default()
    })
}

fn pattern(source: classes::LuaEntity) {
    if pattern_blacklist::check(source.r#type()) {
        return;
    }

    let bb = source.bounding_box();
    let pos = source.position();
    let surface = source.surface();

    let mut offsets_list: Vec<MapPosition> = Vec::new();
    offsets_list.push(MapPosition {
        x: 0.0,
        y: bb.left_top.y - 4.0 - pos.y,
    });
    offsets_list.push(MapPosition {
        x: bb.right_bottom.x + 4.0 - pos.x,
        y: 0.0,
    });
    offsets_list.push(MapPosition {
        x: 0.0,
        y: bb.right_bottom.y + 4.0 - pos.y,
    });
    offsets_list.push(MapPosition {
        x: bb.left_top.x - 4.0 - pos.x,
        y: 0.0,
    });

    for offset in offsets_list {
        let entities = find_pattern(
            source,
            MapPosition {
                x: pos.x + offset.x,
                y: pos.y + offset.y,
            },
        );
        for entity in entities {
            let pos2 = entity.position();
            let third_entities = find_pattern(
                entity,
                MapPosition {
                    x: pos2.x + offset.x,
                    y: pos2.y + offset.y,
                },
            );
            if !third_entities.is_empty() {
                let third = third_entities[0];
                draw_line(
                    surface,
                    ScriptRenderTarget::Position(source.position()),
                    ScriptRenderTarget::Entity(third),
                );
                die(source);
                return;
            } else {
                let back_entities = find_pattern(
                    source,
                    MapPosition {
                        x: pos.x - offset.x,
                        y: pos.y - offset.y,
                    },
                );
                if !back_entities.is_empty() {
                    let third = back_entities[0];
                    draw_line(
                        surface,
                        ScriptRenderTarget::Entity(entity),
                        ScriptRenderTarget::Entity(third),
                    );
                    die(source);
                    return;
                }
            }
        }
    }
}

fn build_handler_impl(source: classes::LuaEntity, player_index: u32) {
    if !source.prototype().is_building() {
        return;
    }

    if ADJACENCY_ENABLED {
        adjacency(source, player_index);
    }

    if !source.valid() {
        return;
    }

    pattern(source);
}

/// Entry point used by event handlers and in-game tests.
pub fn check_build_rules(source: classes::LuaEntity, player_index: u32) {
    build_handler_impl(source, player_index);
}

#[factorio_rs::event]
pub fn on_built_entity(event: OnBuiltEntityEvent) {
    check_build_rules(event.entity, event.player_index);
}

#[factorio_rs::event]
pub fn on_robot_built_entity(event: OnRobotBuiltEntityEvent) {
    check_build_rules(event.entity, 0);
}

#[cfg(test)]
mod tests {
    use factorio_rs::{
        factorio_api::{
            IndexOrName,
            classes::LuaSurfaceCreateEntityParams,
            concepts::{EntityID, ForceID, MapPosition},
        },
        prelude::*,
    };

    use crate::adjacent_blacklist;
    use crate::control;
    use crate::pattern_blacklist;

    fn nauvis() -> factorio_rs::factorio_api::classes::LuaSurface {
        if let Some(surface) = game.get_surface(IndexOrName::Index(1)) {
            return surface;
        }
        panic!("expected nauvis surface at index 1");
    }

    fn place_chest(x: f64, y: f64) -> factorio_rs::factorio_api::classes::LuaEntity {
        let surface = nauvis();
        if let Some(entity) = surface.create_entity(LuaSurfaceCreateEntityParams {
            name: EntityID::Name("iron-chest"),
            position: MapPosition { x, y },
            force: Some(ForceID::Name("player")),
            raise_built: Some(false),
            create_build_effect_smoke: Some(false),
            ..Default::default()
        }) {
            return entity;
        }
        panic!("failed to place iron-chest");
    }

    #[test]
    fn rails_are_on_adjacency_blacklist() {
        assert!(adjacent_blacklist::check("straight-rail"));
        assert!(adjacent_blacklist::check("train-stop"));
        assert!(!adjacent_blacklist::check("assembling-machine-1"));
    }

    #[test]
    fn poles_and_pipes_are_on_pattern_blacklist() {
        assert!(pattern_blacklist::check("electric-pole"));
        assert!(pattern_blacklist::check("pipe"));
        assert!(!pattern_blacklist::check("assembling-machine-1"));
    }

    #[test]
    #[ignore = "requires Factorio (run with factorio-rs test)"]
    fn isolated_building_survives_adjacency_check() {
        let building = place_chest(10.0, 10.0);
        control::check_build_rules(building, 0);
        assert!(building.valid());
    }

    #[test]
    #[ignore = "requires Factorio (run with factorio-rs test)"]
    fn building_survives_with_one_neighbor() {
        let _neighbor = place_chest(20.0, 20.0);
        let building = place_chest(21.0, 20.0);
        control::check_build_rules(building, 0);
        assert!(building.valid());
    }

    #[test]
    #[ignore = "requires Factorio (run with factorio-rs test)"]
    fn building_explodes_with_two_neighbors() {
        // Adjacency rule: more than one neighboring building -> die.
        let _a = place_chest(30.0, 30.0);
        let _b = place_chest(32.0, 30.0);
        let building = place_chest(31.0, 30.0);
        control::check_build_rules(building, 0);
        assert!(!building.valid());
    }

    #[test]
    #[ignore = "requires Factorio (run with factorio-rs test)"]
    fn building_explodes_next_to_three_adjacent_buildings() {
        let _a = place_chest(40.0, 40.0);
        let _b = place_chest(42.0, 40.0);
        let _c = place_chest(41.0, 41.5);
        let building = place_chest(41.0, 40.0);
        control::check_build_rules(building, 0);
        assert!(!building.valid());
    }
}
