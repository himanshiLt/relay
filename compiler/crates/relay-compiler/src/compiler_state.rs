/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::artifact_map::ArtifactMap;
use crate::config::Config;
use crate::errors::{Error, Result};
use crate::file_source::{
    categorize_files, extract_javascript_features_from_file, read_file_to_string, Clock, File,
    FileGroup, FileSourceResult, LocatedDocblockSource, LocatedGraphQLSource,
    LocatedJavascriptSourceFeatures, SourceControlUpdateStatus,
};
use bincode::Options;
use common::{PerfLogEvent, PerfLogger, SourceLocationKey};
use dashmap::DashSet;
use fnv::{FnvBuildHasher, FnvHashMap, FnvHashSet};
use intern::string_key::StringKey;
use rayon::prelude::*;
use relay_config::SchemaConfig;
use relay_transforms::DependencyMap;
use schema::SDLSchema;
use schema_diff::{definitions::SchemaChange, detect_changes};
use serde::{Deserialize, Serialize};
use std::{
    env, fmt,
    fs::File as FsFile,
    hash::Hash,
    io::{BufReader, BufWriter},
    mem,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use std::{slice, vec};
use zstd::stream::{read::Decoder as ZstdDecoder, write::Encoder as ZstdEncoder};

/// Name of a compiler project.
pub type ProjectName = StringKey;

/// Set of project names.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(from = "DeserializableProjectSet")]
pub struct ProjectSet(Vec<ProjectName>);

impl ProjectSet {
    pub fn new(names: Vec<ProjectName>) -> Self {
        if names.is_empty() {
            panic!("Expected project set to have at least one project.")
        }
        ProjectSet(names)
    }
    pub fn of(name: ProjectName) -> Self {
        ProjectSet(vec![name])
    }
    /// Inserts a new project name into this set.
    pub fn insert(&mut self, project_name: ProjectName) {
        let existing_names = &mut self.0;
        assert!(!existing_names.contains(&project_name));
        existing_names.push(project_name);
    }

    pub fn iter(&self) -> slice::Iter<'_, StringKey> {
        self.0.iter()
    }

    pub fn has_multiple_projects(&self) -> bool {
        self.0.len() > 1
    }

    pub fn first(&self) -> Option<&ProjectName> {
        self.0.first()
    }
}

impl IntoIterator for ProjectSet {
    type Item = ProjectName;
    type IntoIter = vec::IntoIter<StringKey>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl fmt::Display for ProjectSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = self
            .0
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<String>>()
            .join(",");

        write!(f, "{}", names)
    }
}

// Serde has the built in ability for deserializing enums that might be in
// different shapes. In this case a `Vec<StringKey>` or `StringKey`. However, we
// want our actual `ProjectSet` object to be modeled as a single Vec internally.
// So, we provide this enum to use sede's polymorphic deserialization and then
// tell `ProjectSet` to deserialize via this enum using `From`.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum DeserializableProjectSet {
    ProjectName(ProjectName),
    ProjectNames(Vec<ProjectName>),
}

impl From<DeserializableProjectSet> for ProjectSet {
    fn from(legacy: DeserializableProjectSet) -> Self {
        match legacy {
            DeserializableProjectSet::ProjectName(name) => ProjectSet::of(name),
            DeserializableProjectSet::ProjectNames(names) => ProjectSet::new(names),
        }
    }
}

pub trait Source {
    fn is_empty(&self) -> bool;
}

impl Source for Vec<LocatedGraphQLSource> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl Source for Vec<LocatedDocblockSource> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

type IncrementalSourceSet<V> = FnvHashMap<PathBuf, V>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IncrementalSources<V: Source> {
    pub pending: IncrementalSourceSet<V>,
    pub processed: IncrementalSourceSet<V>,
}

impl<V: Source> IncrementalSources<V> {
    /// Merges additional pending sources into this states pending sources.
    fn merge_pending_sources(&mut self, additional_pending_sources: IncrementalSourceSet<V>) {
        self.pending.extend(additional_pending_sources.into_iter());
    }

    /// Remove deleted sources from both pending sources and processed sources.
    fn remove_sources(&mut self, removed_sources: &[PathBuf]) {
        for source in removed_sources {
            self.pending.remove(source);
            self.processed.remove(source);
        }
    }

    fn commit_pending_sources(&mut self) {
        for (file_name, pending_graphql_sources) in self.pending.drain() {
            if pending_graphql_sources.is_empty() {
                self.processed.remove(&file_name);
            } else {
                self.processed.insert(file_name, pending_graphql_sources);
            }
        }
    }
    pub fn get_all(&self) -> Vec<(&PathBuf, &V)> {
        let mut sources: Vec<_>;
        if self.pending.is_empty() {
            sources = self.processed.iter().collect();
        } else {
            sources = self.pending.iter().collect();
            for (key, value) in self.processed.iter() {
                if !self.pending.contains_key(key) {
                    sources.push((key, value));
                }
            }
        }
        sources.sort_by_key(|file_content| file_content.0);
        sources
    }
}

impl<V: Source> Default for IncrementalSources<V> {
    fn default() -> Self {
        IncrementalSources {
            pending: FnvHashMap::default(),
            processed: FnvHashMap::default(),
        }
    }
}

type GraphQLSourceSet = IncrementalSourceSet<Vec<LocatedGraphQLSource>>;
type DocblockSourceSet = IncrementalSourceSet<Vec<LocatedDocblockSource>>;
pub type GraphQLSources = IncrementalSources<Vec<LocatedGraphQLSource>>;
pub type SchemaSources = IncrementalSources<String>;
pub type DocblockSources = IncrementalSources<Vec<LocatedDocblockSource>>;

impl Source for String {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl SchemaSources {
    pub fn get_sources(&self) -> Vec<&String> {
        let sources = self.get_all();
        sources.iter().map(|file_content| file_content.1).collect()
    }

    pub fn get_sources_with_location(&self) -> Vec<(&String, SourceLocationKey)> {
        let sources = self.get_all();
        sources
            .iter()
            .map(|file_content| {
                (
                    file_content.1,
                    SourceLocationKey::standalone(file_content.0.to_str().unwrap()),
                )
            })
            .collect()
    }

    pub fn get_old_sources(&self) -> Vec<&String> {
        let mut sources: Vec<_> = self.processed.iter().collect();
        sources.sort_by_key(|file_content| file_content.0);
        sources.iter().map(|file_content| file_content.1).collect()
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub enum ArtifactMapKind {
    /// A simple set of paths of generated files. This kind is used when the
    /// compiler starts without a saved state and doesn't know the connection
    /// between generated files and the artifacts that produced them.
    Unconnected(FnvHashSet<PathBuf>),
    /// A known mapping from input documents to generated outputs.
    Mapping(ArtifactMap),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CompilerState {
    pub graphql_sources: FnvHashMap<ProjectName, GraphQLSources>,
    pub schemas: FnvHashMap<ProjectName, SchemaSources>,
    pub extensions: FnvHashMap<ProjectName, SchemaSources>,
    pub docblocks: FnvHashMap<ProjectName, DocblockSources>,
    pub artifacts: FnvHashMap<ProjectName, Arc<ArtifactMapKind>>,
    // TODO: How can I can I make this just an ImplicitDependencyMap? Currently I can't move the hashmap out of the Arc wrapper around the dirty version.
    pub implicit_dependencies: Arc<RwLock<DependencyMap>>,
    #[serde(with = "clock_json_string")]
    pub clock: Option<Clock>,
    pub saved_state_version: String,
    #[serde(skip)]
    pub dirty_artifact_paths: FnvHashMap<ProjectName, DashSet<PathBuf, FnvBuildHasher>>,
    #[serde(skip)]
    pub pending_implicit_dependencies: Arc<RwLock<DependencyMap>>,
    #[serde(skip)]
    pub pending_file_source_changes: Arc<RwLock<Vec<FileSourceResult>>>,
    #[serde(skip)]
    pub schema_cache: FnvHashMap<ProjectName, Arc<SDLSchema>>,
    #[serde(skip)]
    pub source_control_update_status: Arc<SourceControlUpdateStatus>,
}

impl CompilerState {
    pub fn from_file_source_changes(
        config: &Config,
        file_source_changes: &FileSourceResult,
        setup_event: &impl PerfLogEvent,
        perf_logger: &impl PerfLogger,
    ) -> Result<Self> {
        let categorized = setup_event.time("categorize_files_time", || {
            categorize_files(config, file_source_changes)
        });

        let mut result = Self {
            graphql_sources: Default::default(),
            artifacts: Default::default(),
            implicit_dependencies: Default::default(),
            extensions: Default::default(),
            docblocks: Default::default(),
            schemas: Default::default(),
            clock: file_source_changes.clock(),
            saved_state_version: config.saved_state_version.clone(),
            dirty_artifact_paths: Default::default(),
            pending_implicit_dependencies: Default::default(),
            pending_file_source_changes: Default::default(),
            schema_cache: Default::default(),
            source_control_update_status: Default::default(),
        };

        for (category, files) in categorized {
            match category {
                FileGroup::Source { project_set } => {
                    let (graphql_sources, docblock_sources) = extract_sources(
                        &project_set,
                        files,
                        file_source_changes,
                        false,
                        perf_logger,
                    )?;

                    for project_name in project_set {
                        result.set_pending_source_set(project_name, graphql_sources.clone());
                        result.set_pending_docblock_set(project_name, docblock_sources.clone());
                    }
                }
                FileGroup::Schema { project_set } => {
                    Self::process_schema_change(
                        file_source_changes,
                        files,
                        project_set,
                        &mut result.schemas,
                    )?;
                }
                FileGroup::Extension { project_set } => {
                    Self::process_schema_change(
                        file_source_changes,
                        files,
                        project_set,
                        &mut result.extensions,
                    )?;
                }
                FileGroup::Generated { project_name } => {
                    result.artifacts.insert(
                        project_name,
                        Arc::new(ArtifactMapKind::Unconnected(
                            files.into_iter().map(|file| file.name).collect(),
                        )),
                    );
                }
                FileGroup::Ignore => {}
            }
        }

        Ok(result)
    }

    pub fn project_has_pending_changes(&self, project_name: ProjectName) -> bool {
        self.graphql_sources
            .get(&project_name)
            .map_or(false, |sources| !sources.pending.is_empty())
            || self.project_has_pending_schema_changes(project_name)
            || self.dirty_artifact_paths.contains_key(&project_name)
    }

    pub fn project_has_pending_schema_changes(&self, project_name: ProjectName) -> bool {
        self.schemas
            .get(&project_name)
            .map_or(false, |sources| !sources.pending.is_empty())
            || self
                .extensions
                .get(&project_name)
                .map_or(false, |sources| !sources.pending.is_empty())
            || self
                .docblocks
                .get(&project_name)
                .map_or(false, |sources| !sources.pending.is_empty())
    }

    pub fn has_processed_changes(&self) -> bool {
        self.graphql_sources
            .values()
            .any(|sources| !sources.processed.is_empty())
            || self
                .schemas
                .values()
                .any(|sources| !sources.processed.is_empty())
            || self
                .extensions
                .values()
                .any(|sources| !sources.processed.is_empty())
            || self
                .docblocks
                .values()
                .any(|sources| !sources.processed.is_empty())
    }

    fn is_change_safe(&self, sources: &SchemaSources, schema_config: &SchemaConfig) -> bool {
        let previous = sources
            .get_old_sources()
            .into_iter()
            .map(String::as_str)
            .collect::<Vec<_>>();

        let current = sources
            .get_sources()
            .into_iter()
            .map(String::as_str)
            .collect::<Vec<_>>();

        let schema_change = detect_changes(&current, &previous);

        if schema_change == SchemaChange::None {
            true
        } else {
            match relay_schema::build_schema_with_extensions(
                &current,
                &Vec::<(&str, SourceLocationKey)>::new(),
            ) {
                Ok(schema) => schema_change.is_safe(&schema, schema_config),
                Err(_) => false,
            }
        }
    }

    /// This method will detect any schema changes in the pending sources (for LSP Server, to invalidate schema cache)
    pub fn has_schema_changes(&self) -> bool {
        self.docblocks
            .values()
            .any(|sources| !sources.pending.is_empty())
            || self
                .extensions
                .values()
                .any(|sources| !sources.pending.is_empty())
            || self
                .schemas
                .iter()
                .any(|(_, sources)| !sources.pending.is_empty())
    }

    /// This method is looking at the pending schema changes to see if they may be breaking (removed types, renamed field, etc)
    pub fn has_breaking_schema_change(
        &self,
        project_name: StringKey,
        schema_config: &SchemaConfig,
    ) -> bool {
        if let Some(extension) = self.extensions.get(&project_name) {
            if !extension.pending.is_empty() {
                return true;
            }
        }
        if let Some(docblocks) = self.docblocks.get(&project_name) {
            if !docblocks.pending.is_empty() {
                return true;
            }
        }
        if let Some(schema) = self.schemas.get(&project_name) {
            if !(schema.pending.is_empty() || self.is_change_safe(schema, schema_config)) {
                return true;
            }
        }
        false
    }

    /// Merges pending changes from the file source into the compiler state.
    /// Returns a boolean indicating if any new changes were merged.
    pub fn merge_file_source_changes(
        &mut self,
        config: &Config,
        perf_logger: &impl PerfLogger,
        // When loading from saved state, collect dirty artifacts for recompiling their source definitions
        should_collect_changed_artifacts: bool,
    ) -> Result<bool> {
        let mut has_changed = false;
        for file_source_changes in self.pending_file_source_changes.write().unwrap().drain(..) {
            let log_event = perf_logger.create_event("merge_file_source_changes");
            log_event.number("number_of_changes", file_source_changes.size());
            let categorized = log_event.time("categorize_files_time", || {
                categorize_files(config, &file_source_changes)
            });

            for (category, files) in categorized {
                match category {
                    FileGroup::Source { project_set } => {
                        // TODO: possible optimization to only set this if the
                        // extracted sources actually differ.
                        has_changed = true;

                        let (graphql_sources, docblock_sources) = extract_sources(
                            &project_set,
                            files,
                            &file_source_changes,
                            true,
                            perf_logger,
                        )?;

                        for project_name in project_set {
                            self.graphql_sources
                                .entry(project_name)
                                .or_default()
                                .merge_pending_sources(graphql_sources.clone());
                            self.docblocks
                                .entry(project_name)
                                .or_default()
                                .merge_pending_sources(docblock_sources.clone());
                        }
                    }
                    FileGroup::Schema { project_set } => {
                        has_changed = true;
                        Self::process_schema_change(
                            &file_source_changes,
                            files,
                            project_set,
                            &mut self.schemas,
                        )?;
                    }
                    FileGroup::Extension { project_set } => {
                        has_changed = true;
                        Self::process_schema_change(
                            &file_source_changes,
                            files,
                            project_set,
                            &mut self.extensions,
                        )?;
                    }
                    FileGroup::Generated { project_name } => {
                        if should_collect_changed_artifacts {
                            let mut dashset =
                                DashSet::with_capacity_and_hasher(files.len(), Default::default());
                            dashset.extend(files.into_iter().map(|f| f.name));
                            self.dirty_artifact_paths.insert(project_name, dashset);
                        }
                    }
                    FileGroup::Ignore => {}
                }
            }
        }

        Ok(has_changed)
    }

    pub fn complete_compilation(&mut self) {
        for sources in self.graphql_sources.values_mut() {
            sources.commit_pending_sources();
        }
        for sources in self.schemas.values_mut() {
            sources.commit_pending_sources();
        }
        for sources in self.extensions.values_mut() {
            sources.commit_pending_sources();
        }
        for sources in self.docblocks.values_mut() {
            sources.commit_pending_sources();
        }
        self.implicit_dependencies = mem::take(&mut self.pending_implicit_dependencies);
        self.dirty_artifact_paths.clear();
    }

    pub fn complete_project_compilation(&mut self, project_name: &ProjectName) {
        if let Some(sources) = self.graphql_sources.get_mut(project_name) {
            sources.commit_pending_sources();
        }
        if let Some(sources) = self.schemas.get_mut(project_name) {
            sources.commit_pending_sources();
        }
        if let Some(sources) = self.extensions.get_mut(project_name) {
            sources.commit_pending_sources();
        }
        if let Some(sources) = self.docblocks.get_mut(project_name) {
            sources.commit_pending_sources();
        }
    }

    /// Calculate dirty definitions from dirty artifacts
    pub fn get_dirty_definitions(
        &self,
        config: &Config,
    ) -> FnvHashMap<ProjectName, Vec<StringKey>> {
        if self.dirty_artifact_paths.is_empty() {
            return Default::default();
        }
        let mut result = FnvHashMap::default();
        for (project_name, paths) in self.dirty_artifact_paths.iter() {
            if config.projects[project_name].enabled {
                let paths = paths.clone();
                let artifacts = self
                    .artifacts
                    .get(project_name)
                    .expect("Expected the artifacts map to exist.");
                if let ArtifactMapKind::Mapping(artifacts) = &**artifacts {
                    let mut dirty_definitions = vec![];
                    'outer: for entry in artifacts.0.iter() {
                        let (definition_name, artifact_records) = entry.pair();
                        let mut added = false;
                        for artifact_record in artifact_records {
                            if paths.remove(&artifact_record.path).is_some() && !added {
                                dirty_definitions.push(*definition_name);
                                if paths.is_empty() {
                                    break 'outer;
                                }
                                added = true;
                            }
                        }
                    }
                    if !dirty_definitions.is_empty() {
                        result.insert(*project_name, dirty_definitions);
                    }
                } else {
                    panic!("Expected the artifacts map to be populated.")
                }
            }
        }
        result
    }

    pub fn serialize_to_file(&self, path: &PathBuf) -> Result<()> {
        let writer = FsFile::create(path)
            .and_then(|writer| ZstdEncoder::new(writer, 12))
            .map_err(|err| Error::WriteFileError {
                file: path.clone(),
                source: err,
            })?
            .auto_finish();
        let writer =
            BufWriter::with_capacity(ZstdEncoder::<FsFile>::recommended_input_size(), writer);
        bincode::serialize_into(writer, self).map_err(|err| Error::SerializationError {
            file: path.clone(),
            source: err,
        })
    }

    pub fn deserialize_from_file(path: &PathBuf) -> Result<Self> {
        let reader = FsFile::open(path)
            .and_then(ZstdDecoder::new)
            .map_err(|err| Error::ReadFileError {
                file: path.clone(),
                source: err,
            })?;
        let reader = BufReader::with_capacity(
            ZstdDecoder::<BufReader<FsFile>>::recommended_output_size(),
            reader,
        );

        let memory_limit: u64 = env::var("RELAY_SAVED_STATE_MEMORY_LIMIT")
            .map(|limit| {
                limit.parse::<u64>().expect(
                    "Expected RELAY_SAVED_STATE_MEMORY_LIMIT environment variable to be a number.",
                )
            })
            .unwrap_or_else(|_| 10_u64.pow(10) /* 10GB */);

        bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .allow_trailing_bytes()
            .with_limit(memory_limit)
            .deserialize_from(reader)
            .map_err(|err| Error::DeserializationError {
                file: path.clone(),
                source: err,
            })
    }

    pub fn has_pending_file_source_changes(&self) -> bool {
        !self.pending_file_source_changes.read().unwrap().is_empty()
    }

    fn set_pending_source_set(&mut self, project_name: ProjectName, source_set: GraphQLSourceSet) {
        let pending_entry = &mut self
            .graphql_sources
            .entry(project_name)
            .or_default()
            .pending;
        if pending_entry.is_empty() {
            *pending_entry = source_set;
        } else {
            pending_entry.extend(source_set);
        }
    }

    fn set_pending_docblock_set(
        &mut self,
        project_name: ProjectName,
        source_set: DocblockSourceSet,
    ) {
        let pending_entry = &mut self.docblocks.entry(project_name).or_default().pending;
        if pending_entry.is_empty() {
            *pending_entry = source_set;
        } else {
            pending_entry.extend(source_set);
        }
    }

    fn process_schema_change(
        file_source_changes: &FileSourceResult,
        files: Vec<File>,
        project_set: ProjectSet,
        source_map: &mut FnvHashMap<ProjectName, SchemaSources>,
    ) -> Result<()> {
        let mut removed_sources = vec![];
        let mut added_sources = FnvHashMap::default();
        for file in files {
            let file_name = file.name.clone();
            if file.exists {
                added_sources.insert(file_name, read_file_to_string(file_source_changes, &file)?);
            } else {
                removed_sources.push(file_name);
            }
        }
        for project_name in project_set {
            let entry = source_map.entry(project_name).or_default();
            entry.remove_sources(&removed_sources);
            entry.merge_pending_sources(added_sources.clone());
        }
        Ok(())
    }

    pub fn is_source_control_update_in_progress(&self) -> bool {
        self.source_control_update_status.is_started()
    }

    /// Over the course of the build, we may need to stop current progress
    /// as there maybe incoming file change or source control update in progress
    pub fn should_cancel_current_build(&self) -> bool {
        self.is_source_control_update_in_progress() || self.has_pending_file_source_changes()
    }
}

fn extract_sources(
    project_set: &ProjectSet,
    files: Vec<File>,
    file_source_changes: &FileSourceResult,
    preserve_empty: bool,
    perf_logger: &impl PerfLogger,
) -> Result<(GraphQLSourceSet, DocblockSourceSet)> {
    let log_event = perf_logger.create_event("categorize");
    log_event.string("source_set_name", project_set.to_string());
    let extract_timer = log_event.start("extract_graphql_strings_from_file_time");

    let source_features = files
        .par_iter()
        .map(|file| {
            if file.exists {
                match extract_javascript_features_from_file(file_source_changes, file) {
                    Ok(features) => Ok((file, features)),
                    Err(err) => Err(err),
                }
            } else {
                Ok((file, LocatedJavascriptSourceFeatures::default()))
            }
        })
        .collect::<Result<Vec<_>>>()?;

    log_event.stop(extract_timer);

    let mut graphql_sources: GraphQLSourceSet = FnvHashMap::default();
    let mut docblock_sources: DocblockSourceSet = FnvHashMap::default();
    for (file, features) in source_features {
        if preserve_empty || !features.graphql_sources.is_empty() {
            graphql_sources.insert(file.name.clone(), features.graphql_sources);
        }
        if preserve_empty || !features.docblock_sources.is_empty() {
            docblock_sources.insert(file.name.clone(), features.docblock_sources);
        }
    }

    Ok((graphql_sources, docblock_sources))
}

/// A module to serialize a watchman Clock value via JSON.
/// The reason is that `Clock` internally uses an untagged enum value
/// which requires "self descriptive" serialization formats and `bincode` does not
/// support those enums.
mod clock_json_string {
    use crate::file_source::Clock;
    use serde::{
        de::{Error, Visitor},
        Deserializer, Serializer,
    };

    pub fn serialize<S>(clock: &Option<Clock>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let json_string = serde_json::to_string(clock).unwrap();
        serializer.serialize_str(&json_string)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Clock>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(JSONStringVisitor)
    }

    struct JSONStringVisitor;
    impl<'de> Visitor<'de> for JSONStringVisitor {
        type Value = Option<Clock>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a JSON encoded watchman::Clock value")
        }

        fn visit_str<E: Error>(self, v: &str) -> Result<Option<Clock>, E> {
            Ok(serde_json::from_str(v).unwrap())
        }
    }
}
