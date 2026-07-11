#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::doc_markdown,
    clippy::unnested_or_patterns,
    clippy::option_if_let_else,
    clippy::map_unwrap_or,
    clippy::needless_pass_by_value,
    clippy::use_self,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::match_same_arms,
    clippy::items_after_statements,
    clippy::struct_field_names,
    clippy::if_not_else,
    clippy::from_iter_instead_of_collect,
    clippy::manual_string_new,
    clippy::wildcard_imports,
    clippy::enum_glob_use,
    clippy::unused_self,
    clippy::trivially_copy_pass_by_ref,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::integer_division,
    clippy::arithmetic_side_effects,
    clippy::missing_const_for_fn,
    clippy::implicit_hasher,
    clippy::semicolon_if_nothing_returned,
    clippy::redundant_closure_for_method_calls,
    clippy::unnecessary_wraps,
    clippy::return_self_not_must_use,
    clippy::pub_use,
    clippy::module_inception,
    clippy::redundant_pub_crate
)]

mod generate;
mod schema;

use std::path::Path;

pub use schema::RuntimeApi;

pub struct GeneratedApi {
    pub application_version: String,
    pub api_version: u32,
    pub events: String,
    pub event_map: String,
    pub event_lookup: String,
    pub event_filter_lookup: String,
    pub event_module_lookup: String,
    pub event_filters: String,
    pub event_data: String,
    pub defines: String,
    pub classes: String,
    pub globals: String,
    pub concepts: String,
    pub unions: String,
}

pub fn parse_runtime_api(json: &str) -> Result<RuntimeApi, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn generate_runtime_api(api: &RuntimeApi) -> GeneratedApi {
    let mappings = generate::collect_event_mappings(api);
    let class_names = generate::class_names(api);
    let filter_concept_names = generate::event_filter_concept_names(api);
    let identification_names = generate::identification_concept_names(api, &filter_concept_names);
    let identification_signatures = generate::identification_signatures(api, &identification_names);
    let mut concept_names = generate::generatable_concept_names(api, &filter_concept_names);
    concept_names.extend(identification_names.iter().cloned());
    let union_registry = generate::collect_literal_unions(api);
    let known = generate::KnownTypes {
        classes: &class_names,
        concepts: &concept_names,
        identifications: &identification_names,
        identification_signatures: &identification_signatures,
        unions: union_registry.names(),
        union_registry: &union_registry,
    };
    let event_filters = generate::generate_event_filters(api);
    let concepts = {
        let mut structs = generate::generate_concepts(api, &known, &filter_concept_names);
        let ids = generate::generate_identifications(api, &known).to_string();
        structs.push_str(&ids);
        structs
    };

    GeneratedApi {
        application_version: api.application_version.clone(),
        api_version: api.api_version,
        events: generate::generate_events(api),
        event_map: generate::generate_event_map(&mappings),
        event_lookup: generate::generate_event_lookup(&mappings),
        event_filter_lookup: generate::generate_event_filter_lookup(&mappings),
        event_module_lookup: generate::generate_event_module_lookup(&mappings),
        event_filters,
        event_data: generate::generate_event_data(api, &known),
        defines: generate::generate_defines(&api.defines),
        classes: generate::generate_classes(api, &known),
        globals: generate::generate_globals(api, &known),
        concepts,
        unions: generate::generate_unions(&union_registry),
    }
}

pub fn generate_from_json(json: &str) -> Result<GeneratedApi, serde_json::Error> {
    let api = parse_runtime_api(json)?;
    Ok(generate_runtime_api(&api))
}

pub fn bundled_runtime_api_json() -> &'static str {
    include_str!("../api/runtime-api.json")
}

pub fn generate_from_bundled_api() -> Result<GeneratedApi, serde_json::Error> {
    generate_from_json(bundled_runtime_api_json())
}

pub fn write_generated_api(output_dir: &Path, generated: &GeneratedApi) -> std::io::Result<()> {
    std::fs::create_dir_all(output_dir)?;

    write_module(
        output_dir,
        "mod.rs",
        &format!(
            "// Generated from Factorio runtime API v{} (format v{}).\n\
             #[allow(unused, clippy::all, clippy::pedantic, clippy::nursery)]\n\n\
             pub mod classes;\n\
             pub mod concepts;\n\
             pub mod defines;\n\
             pub mod event_data;\n\
             pub mod event_filters;\n\
             pub mod events;\n\
             pub mod globals;\n\
             pub mod map;\n\
             pub mod unions;\n",
            generated.application_version, generated.api_version
        ),
    )?;
    write_module(output_dir, "events.rs", &generated.events)?;
    write_module(output_dir, "map.rs", &generated.event_map)?;
    write_module(output_dir, "event_filters.rs", &generated.event_filters)?;
    write_module(output_dir, "event_data.rs", &generated.event_data)?;
    write_module(output_dir, "defines.rs", &generated.defines)?;
    write_module(output_dir, "classes.rs", &generated.classes)?;
    write_module(output_dir, "globals.rs", &generated.globals)?;
    write_module(output_dir, "concepts.rs", &generated.concepts)?;
    write_module(output_dir, "unions.rs", &generated.unions)?;

    Ok(())
}

pub fn write_macro_event_lookup(
    output_dir: &Path,
    generated: &GeneratedApi,
) -> std::io::Result<()> {
    std::fs::create_dir_all(output_dir)?;
    write_module(output_dir, "event_lookup.rs", &generated.event_lookup)?;
    write_module(
        output_dir,
        "event_filter_lookup.rs",
        &generated.event_filter_lookup,
    )?;
    write_module(
        output_dir,
        "event_module_lookup.rs",
        &generated.event_module_lookup,
    )
}

fn write_module(output_dir: &Path, file_name: &str, contents: &str) -> std::io::Result<()> {
    let path = output_dir.join(file_name);
    std::fs::write(path, contents)
}
