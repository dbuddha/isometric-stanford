//! User-facing orchestration commands.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use isometric_publish::{DziOptions, InputDigests, sha256_hex, validate_dzi};
use isometric_render::{render_reference, render_world, stable_hash};
use isometric_style::StylePack;
use isometric_validate::{validate_style, validate_world};
use isometric_world::World;

const USAGE: &str = "Usage:
  isometric-stanford source sync [cache-directory]
  isometric-stanford perceive run
  isometric-stanford world compile [output-directory]
  isometric-stanford world inspect [world.json]
  isometric-stanford render region [output.ppm]
  isometric-stanford render fixture [output.ppm]
  isometric-stanford render slice
  isometric-stanford validate semantic|render
  isometric-stanford validate release [artifact-directory]
  isometric-stanford publish dzi [output-directory]

Implemented commands:
  render region writes the compiled deterministic hero-world PPM.
  render fixture writes the original synthetic regression PPM.
  validate semantic, validate render, and validate release are executable.
  source sync verifies approved artifacts in a content-addressed cache.
  world compile verifies the complete source lock, compiles the locked vectors,
  and writes a canonical world plus manifest. world inspect validates an artifact.
  publish dzi writes a staged, lossless WebP DZI and indexed canonical pyramid.
  Other commands fail closed until their tracked task is implemented.";

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<String, String> {
    match arguments {
        [group, command] if group == "source" && command == "sync" => {
            sync_sources(Path::new("artifacts/source-cache"))
        }
        [group, command, cache] if group == "source" && command == "sync" => {
            sync_sources(&PathBuf::from(cache))
        }
        [group, command] if group == "world" && command == "compile" => {
            compile_world(Path::new("artifacts/world"))
        }
        [group, command, output] if group == "world" && command == "compile" => {
            compile_world(Path::new(output))
        }
        [group, command] if group == "world" && command == "inspect" => {
            inspect_world(Path::new("artifacts/world/hero.json"))
        }
        [group, command, input] if group == "world" && command == "inspect" => {
            inspect_world(Path::new(input))
        }
        [group, command] if group == "validate" && command == "semantic" => {
            validate_world(&World::reference_fixture()).map_err(|error| error.to_string())?;
            Ok("semantic fixture passed".into())
        }
        [group, command] if group == "validate" && command == "render" => {
            let world = World::reference_fixture();
            let style = StylePack::stanford_v1();
            validate_world(&world).map_err(|error| error.to_string())?;
            validate_style(&style).map_err(|error| error.to_string())?;
            let image =
                render_reference(&world, &style, 128, 128).map_err(|error| error.to_string())?;
            Ok(format!(
                "reference render passed: {:016x}",
                stable_hash(image.pixels())
            ))
        }
        [group, command] if group == "validate" && command == "release" => {
            validate_release(Path::new("artifacts/dzi/hero"))
        }
        [group, command, input] if group == "validate" && command == "release" => {
            validate_release(Path::new(input))
        }
        [group, command] if group == "render" && command == "region" => {
            render_region("artifacts/render/hero.ppm")
        }
        [group, command, output] if group == "render" && command == "region" => {
            render_region(output)
        }
        [group, command] if group == "render" && command == "fixture" => {
            render_fixture("artifacts/reference.ppm")
        }
        [group, command, output] if group == "render" && command == "fixture" => {
            render_fixture(output)
        }
        [group, command] if group == "publish" && command == "dzi" => {
            publish_dzi_artifact(Path::new("artifacts/dzi/hero"))
        }
        [group, command, output] if group == "publish" && command == "dzi" => {
            publish_dzi_artifact(Path::new(output))
        }
        [] => Ok(USAGE.into()),
        [single] if single == "--help" || single == "-h" => Ok(USAGE.into()),
        [group, command] => Err(format!(
            "{group} {command} is specified but not implemented yet"
        )),
        _ => Err("unrecognized command".into()),
    }
}

fn sync_sources(cache: &Path) -> Result<String, String> {
    let artifacts = isometric_source::sync(Path::new("source.lock.json"), cache)
        .map_err(|error| error.to_string())?;
    let downloaded = artifacts.iter().filter(|artifact| !artifact.reused).count();
    Ok(format!(
        "verified {} sources in {} ({downloaded} downloaded)",
        artifacts.len(),
        cache.display()
    ))
}

fn compile_world(output: &Path) -> Result<String, String> {
    let artifacts = isometric_source::sync_selected(
        Path::new("source.lock.json"),
        Path::new("artifacts/source-cache"),
        &["osm-2026-07-15-hero", "overture-2026-06-17-buildings"],
    )
    .map_err(|error| error.to_string())?;
    let path_for = |id: &str| {
        artifacts
            .iter()
            .find(|artifact| artifact.id == id)
            .map(|artifact| artifact.path.as_path())
            .ok_or_else(|| format!("verified source cache lacks {id}"))
    };
    let osm = fs::read(path_for("osm-2026-07-15-hero")?).map_err(|error| error.to_string())?;
    let overture =
        fs::read(path_for("overture-2026-06-17-buildings")?).map_err(|error| error.to_string())?;
    let compiled =
        isometric_world::compile_hero(&osm, &overture).map_err(|error| error.to_string())?;
    fs::create_dir_all(output).map_err(|error| error.to_string())?;
    atomic_write(&output.join("hero.json"), compiled.world_json.as_bytes())?;
    atomic_write(
        &output.join("world.manifest.json"),
        compiled.manifest_json.as_bytes(),
    )?;
    Ok(format!(
        "compiled {} objects into {} partitions at {} (unknown {} ppm; rejected {} source geometries)",
        compiled.report.object_count,
        compiled.world.partitions().len(),
        output.display(),
        compiled.report.unknown_fraction_ppm,
        compiled.report.rejected_geometry_count
    ))
}

fn inspect_world(input: &Path) -> Result<String, String> {
    let json = fs::read_to_string(input).map_err(|error| error.to_string())?;
    let world = World::from_artifact_json(&json).map_err(|error| error.to_string())?;
    let named = world
        .objects()
        .iter()
        .filter(|object| object.name().is_some())
        .count();
    Ok(format!(
        "validated {} objects ({} named) across {} partitions from {} sources",
        world.objects().len(),
        named,
        world.partitions().len(),
        world.sources().len()
    ))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("artifact");
    let temporary = path.with_extension(format!("{extension}.partial"));
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn render_region(output: &str) -> Result<String, String> {
    let json = fs::read_to_string("artifacts/world/hero.json").map_err(|error| {
        format!("read artifacts/world/hero.json after `world compile`: {error}")
    })?;
    let world = World::from_artifact_json(&json).map_err(|error| error.to_string())?;
    let style = StylePack::stanford_v1();
    let image = render_world(&world, &style).map_err(|error| error.to_string())?;
    write_ppm(output, &image, &style)
}

fn render_fixture(output: &str) -> Result<String, String> {
    let world = World::reference_fixture();
    let style = StylePack::stanford_v1();
    let image = render_reference(&world, &style, 256, 256).map_err(|error| error.to_string())?;
    write_ppm(output, &image, &style)
}

fn publish_dzi_artifact(output: &Path) -> Result<String, String> {
    let world_bytes = fs::read("artifacts/world/hero.json")
        .map_err(|error| format!("read artifacts/world/hero.json after world compile: {error}"))?;
    let world_json = std::str::from_utf8(&world_bytes).map_err(|error| error.to_string())?;
    let world = World::from_artifact_json(world_json).map_err(|error| error.to_string())?;
    let style_bytes =
        fs::read("styles/stanford_v1/style.toml").map_err(|error| error.to_string())?;
    let inputs = InputDigests::new(sha256_hex(&world_bytes), sha256_hex(&style_bytes))
        .map_err(|error| error.to_string())?;
    let report = isometric_publish::publish_dzi(
        &world,
        &StylePack::stanford_v1(),
        &inputs,
        output,
        DziOptions::prototype(),
    )
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "published {} x {} DZI with {} tiles and {} encoded bytes at {}: {}",
        report.width,
        report.height,
        report.tile_count,
        report.encoded_bytes,
        output.display(),
        report.tile_set_sha256
    ))
}

fn validate_release(input: &Path) -> Result<String, String> {
    let report = validate_dzi(input, &StylePack::stanford_v1().palette)
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "validated {} x {} DZI with {} tiles and {} encoded bytes at {}: {}",
        report.width,
        report.height,
        report.tile_count,
        report.encoded_bytes,
        input.display(),
        report.tile_set_sha256
    ))
}

fn write_ppm(
    output: &str,
    image: &isometric_render::IndexedImage,
    style: &StylePack,
) -> Result<String, String> {
    let bytes = image
        .to_ppm(&style.palette)
        .map_err(|error| error.to_string())?;
    if let Some(parent) = std::path::Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(output, bytes).map_err(|error| error.to_string())?;
    Ok(format!(
        "wrote {output} ({} x {}): {:016x}",
        image.width(),
        image.height(),
        stable_hash(image.pixels())
    ))
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn semantic_validation_command_runs() {
        let arguments = vec!["validate".into(), "semantic".into()];
        let result = run(&arguments);
        assert_eq!(
            result.expect("command must pass"),
            "semantic fixture passed"
        );
    }

    #[test]
    fn unimplemented_contract_fails_closed() {
        let arguments = vec!["perceive".into(), "run".into()];
        let result = run(&arguments);
        assert!(result.expect_err("must fail").contains("not implemented"));
    }
}
