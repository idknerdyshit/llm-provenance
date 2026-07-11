//! Hash-bound descriptions of application-owned context reconstruction inputs.

use serde::{Deserialize, Serialize};

use crate::{ArtifactDigest, OpaqueIdentifier, Result};

/// Stable identity and application-defined version for a reconstruction or
/// execution component.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct VersionedComponent {
    id: OpaqueIdentifier,
    version: OpaqueIdentifier,
}

impl VersionedComponent {
    /// Construct a component identity from validated opaque values.
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Result<Self> {
        Ok(Self {
            id: OpaqueIdentifier::new(id)?,
            version: OpaqueIdentifier::new(version)?,
        })
    }

    /// Return the component identifier.
    #[must_use]
    pub fn id(&self) -> &OpaqueIdentifier {
        &self.id
    }

    /// Return the application-defined component version.
    #[must_use]
    pub fn version(&self) -> &OpaqueIdentifier {
        &self.version
    }
}

impl<'de> Deserialize<'de> for VersionedComponent {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            id: OpaqueIdentifier,
            version: OpaqueIdentifier,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            id: wire.id,
            version: wire.version,
        })
    }
}

/// One ordered immutable application source used to build a context.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct SourceSnapshot {
    source_id: OpaqueIdentifier,
    revision: OpaqueIdentifier,
    content_digest: ArtifactDigest,
}

impl SourceSnapshot {
    /// Construct a snapshot commitment.
    pub fn new(
        source_id: impl Into<String>,
        revision: impl Into<String>,
        content_digest: ArtifactDigest,
    ) -> Result<Self> {
        Ok(Self {
            source_id: OpaqueIdentifier::new(source_id)?,
            revision: OpaqueIdentifier::new(revision)?,
            content_digest,
        })
    }

    /// Return the application-defined logical source identifier.
    #[must_use]
    pub fn source_id(&self) -> &OpaqueIdentifier {
        &self.source_id
    }

    /// Return the immutable application-defined revision identifier.
    #[must_use]
    pub fn revision(&self) -> &OpaqueIdentifier {
        &self.revision
    }

    /// Return the exact-byte commitment for the retained snapshot content.
    #[must_use]
    pub fn content_digest(&self) -> ArtifactDigest {
        self.content_digest
    }
}

impl<'de> Deserialize<'de> for SourceSnapshot {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            source_id: OpaqueIdentifier,
            revision: OpaqueIdentifier,
            content_digest: ArtifactDigest,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            source_id: wire.source_id,
            revision: wire.revision,
            content_digest: wire.content_digest,
        })
    }
}

/// Ordered, hashable reconstruction metadata for an application context.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct ContextManifest {
    sources: Vec<SourceSnapshot>,
    retrieval_config: ArtifactDigest,
    selection_config: ArtifactDigest,
    builder: VersionedComponent,
}

impl ContextManifest {
    /// Construct reconstruction metadata. Empty source lists are valid for
    /// contexts assembled entirely from non-retrieval state.
    #[must_use]
    pub fn new(
        sources: Vec<SourceSnapshot>,
        retrieval_config: ArtifactDigest,
        selection_config: ArtifactDigest,
        builder: VersionedComponent,
    ) -> Self {
        Self {
            sources,
            retrieval_config,
            selection_config,
            builder,
        }
    }

    /// Return sources in the exact application-defined assembly order.
    #[must_use]
    pub fn sources(&self) -> &[SourceSnapshot] {
        &self.sources
    }

    /// Return the retrieval configuration commitment.
    #[must_use]
    pub fn retrieval_config(&self) -> ArtifactDigest {
        self.retrieval_config
    }

    /// Return the selection/ranking configuration commitment.
    #[must_use]
    pub fn selection_config(&self) -> ArtifactDigest {
        self.selection_config
    }

    /// Return the context-builder identity.
    #[must_use]
    pub fn builder(&self) -> &VersionedComponent {
        &self.builder
    }
}

impl<'de> Deserialize<'de> for ContextManifest {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            sources: Vec<SourceSnapshot>,
            retrieval_config: ArtifactDigest,
            selection_config: ArtifactDigest,
            builder: VersionedComponent,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self::new(
            wire.sources,
            wire.retrieval_config,
            wire.selection_config,
            wire.builder,
        ))
    }
}

/// Application payload paired with its hash-bound reconstruction manifest.
///
/// Use this as the payload of [`crate::Context`] when context reconstruction is
/// required. Both the manifest and resolved application payload become inputs
/// to the context digest.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestedPayload<T> {
    manifest: ContextManifest,
    payload: T,
}

impl<T> ManifestedPayload<T> {
    /// Pair a reconstruction manifest with resolved application payload data.
    #[must_use]
    pub fn new(manifest: ContextManifest, payload: T) -> Self {
        Self { manifest, payload }
    }

    /// Return the hash-bound reconstruction manifest.
    #[must_use]
    pub fn manifest(&self) -> &ContextManifest {
        &self.manifest
    }

    /// Return the resolved application payload.
    #[must_use]
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// Consume the wrapper and return its manifest and resolved payload.
    #[must_use]
    pub fn into_parts(self) -> (ContextManifest, T) {
        (self.manifest, self.payload)
    }
}
