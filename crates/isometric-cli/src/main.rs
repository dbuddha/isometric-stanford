//! User-facing orchestration commands.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use isometric_render::{render_reference, stable_hash};
use isometric_style::StylePack;
use isometric_validate::{validate_style, validate_world};
use isometric_world::World;

const USAGE: &str = "Usage:
  isometric-stanford source sync [cache-directory]
  isometric-stanford perceive run
  isometric-stanford world compile|inspect
  isometric-stanford render region [output.ppm]
  isometric-stanford render slice
  isometric-stanford validate semantic|render|release
  isometric-stanford publish dzi

Bootstrap implementation:
  render region writes an original deterministic reference PPM.
  validate semantic and validate render are executable.
  source sync verifies approved artifacts in a content-addressed cache.
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
        [group, command] if group == "render" && command == "region" => {
            render_region("artifacts/reference.ppm")
        }
        [group, command, output] if group == "render" && command == "region" => {
            render_region(output)
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

fn render_region(output: &str) -> Result<String, String> {
    let world = World::reference_fixture();
    let style = StylePack::stanford_v1();
    let image = render_reference(&world, &style, 256, 256).map_err(|error| error.to_string())?;
    let bytes = image
        .to_ppm(&style.palette)
        .map_err(|error| error.to_string())?;
    if let Some(parent) = std::path::Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(output, bytes).map_err(|error| error.to_string())?;
    Ok(format!(
        "wrote {output}: {:016x}",
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
        let arguments = vec!["publish".into(), "dzi".into()];
        let result = run(&arguments);
        assert!(result.expect_err("must fail").contains("not implemented"));
    }
}
