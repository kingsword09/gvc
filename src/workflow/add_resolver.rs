use crate::agents::AddTargetKind;
use crate::agents::catalog_editor::{parse_library_coordinate, parse_plugin_coordinate};
use crate::error::{GvcError, Result};
use crate::gradle::Repository;
use crate::repository::{Coordinate, DefaultVersionStrategy, RepositoryFactory, VersionStrategy};
use colored::Colorize;

pub(super) fn resolve_add_coordinate(
    target: AddTargetKind,
    coordinate: &str,
    repositories: &[Repository],
    stable_only: bool,
) -> Result<String> {
    match target {
        AddTargetKind::Library => {
            let (group, artifact, version) = parse_library_coordinate(coordinate)?;
            let resolved_version =
                resolve_library_version(repositories, &group, &artifact, version, stable_only)?;
            Ok(format!("{}:{}:{}", group, artifact, resolved_version))
        }
        AddTargetKind::Plugin => {
            let (plugin_id, version) = parse_plugin_coordinate(coordinate)?;
            let resolved_version = resolve_plugin_version(&plugin_id, version, stable_only)?;
            Ok(format!("{}:{}", plugin_id, resolved_version))
        }
    }
}

fn resolve_library_version(
    repositories: &[Repository],
    group: &str,
    artifact: &str,
    version: String,
    stable_only: bool,
) -> Result<String> {
    let client = RepositoryFactory::create_maven(repositories.to_vec())?;
    let coordinate = Coordinate::new(group, artifact);
    let available_versions = client.fetch_available_versions(&coordinate)?;
    let target_version =
        select_requested_version(&available_versions, version, stable_only).map_err(|message| {
            if stable_only && message == SelectVersionError::NoStableVersion {
                GvcError::ProjectValidation(format!(
                    "No stable versions available for '{}:{}'. Re-run with --no-stable-only to allow pre-releases.",
                    group, artifact
                ))
            } else if message == SelectVersionError::NoVersion {
                GvcError::ProjectValidation(format!(
                    "No versions found for '{}:{}' in the configured repositories",
                    group, artifact
                ))
            } else {
                GvcError::ProjectValidation(format!(
                    "Version '{}' for '{}:{}' not found in configured repositories",
                    message.requested_version(),
                    group,
                    artifact
                ))
            }
        })?;

    crate::outln!(
        "   {}",
        format!("✓ {group}:{artifact} @ {target_version}").green()
    );

    Ok(target_version)
}

fn resolve_plugin_version(plugin_id: &str, version: String, stable_only: bool) -> Result<String> {
    let client = RepositoryFactory::create_plugin_portal()?;
    let coordinate = Coordinate::plugin(plugin_id);
    let available_versions = client.fetch_available_versions(&coordinate)?;
    let target_version =
        select_requested_version(&available_versions, version, stable_only).map_err(|message| {
            if stable_only && message == SelectVersionError::NoStableVersion {
                GvcError::ProjectValidation(format!(
                    "No stable versions available for plugin '{}'. Re-run with --no-stable-only to include pre-releases.",
                    plugin_id
                ))
            } else if message == SelectVersionError::NoVersion {
                GvcError::ProjectValidation(format!(
                    "No versions found for plugin '{}' on Gradle Plugin Portal",
                    plugin_id
                ))
            } else {
                GvcError::ProjectValidation(format!(
                    "Version '{}' for plugin '{}' not found on Gradle Plugin Portal",
                    message.requested_version(),
                    plugin_id
                ))
            }
        })?;

    crate::outln!(
        "   {}",
        format!("✓ plugin {plugin_id} @ {target_version}").green()
    );

    Ok(target_version)
}

fn select_requested_version(
    available_versions: &[String],
    requested_version: String,
    stable_only: bool,
) -> std::result::Result<String, SelectVersionError> {
    if available_versions.is_empty() {
        return Err(SelectVersionError::NoVersion);
    }

    if requested_version.eq_ignore_ascii_case("latest") {
        return DefaultVersionStrategy
            .select_latest(available_versions, stable_only)
            .ok_or(SelectVersionError::NoStableVersion);
    }

    if available_versions
        .iter()
        .any(|available| available == &requested_version)
    {
        Ok(requested_version)
    } else {
        Err(SelectVersionError::MissingRequested(requested_version))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum SelectVersionError {
    NoVersion,
    NoStableVersion,
    MissingRequested(String),
}

impl SelectVersionError {
    fn requested_version(&self) -> &str {
        match self {
            Self::MissingRequested(version) => version,
            Self::NoVersion | Self::NoStableVersion => "latest",
        }
    }
}
