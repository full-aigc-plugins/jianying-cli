use jianying_media::{
    AsrArtifact, AsrError, AsrOutputFormat, AsrProvider, AsrProviderCapability, AsrRequest,
    AsrTimestampGranularity,
};
use std::path::{Path, PathBuf};

struct FixtureAsrProvider {
    capability: AsrProviderCapability,
}

impl AsrProvider for FixtureAsrProvider {
    fn capability(&self) -> &AsrProviderCapability {
        &self.capability
    }

    fn transcribe(
        &self,
        request: &AsrRequest,
        source: &Path,
        output: &Path,
    ) -> Result<AsrArtifact, AsrError> {
        self.capability.validate(request)?;
        if !source.is_file() {
            return Err(AsrError::SourceMissing(source.to_path_buf()));
        }
        std::fs::write(output, b"{\"text\":\"fixture transcript\"}")
            .map_err(|error| AsrError::Io(error.to_string()))?;
        AsrArtifact::from_path(
            output,
            request.output_format(),
            self.capability.provider_id(),
            request.source_sha256(),
        )
    }
}

fn root() -> PathBuf {
    std::env::temp_dir().join(format!("jianying-asr-provider-{}", std::process::id()))
}

#[test]
fn provider_contract_returns_source_bound_transcript_artifact() {
    let root = root();
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("speech.wav");
    let output = root.join("transcript.json");
    std::fs::write(&source, b"synthetic audio").unwrap();
    let source_sha256 = AsrRequest::hash_source(&source).unwrap();
    let request = AsrRequest::new(source_sha256.clone(), AsrOutputFormat::VerboseJson)
        .unwrap()
        .with_model("whisper-large-v3")
        .with_language("zh")
        .with_timestamps([
            AsrTimestampGranularity::Segment,
            AsrTimestampGranularity::Word,
        ]);
    let provider = FixtureAsrProvider {
        capability: AsrProviderCapability::new(
            "fixture-whisper",
            "fixture-whisper@sha256:abc123",
            false,
            [AsrOutputFormat::VerboseJson, AsrOutputFormat::Srt],
            [
                AsrTimestampGranularity::Segment,
                AsrTimestampGranularity::Word,
            ],
        )
        .unwrap(),
    };

    let artifact = provider.transcribe(&request, &source, &output).unwrap();
    assert_eq!(artifact.provider_id(), "fixture-whisper");
    assert_eq!(artifact.source_sha256(), source_sha256);
    assert_eq!(artifact.output_format(), AsrOutputFormat::VerboseJson);
    assert!(artifact.byte_length() > 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn timestamps_fail_closed_for_non_verbose_or_unsupported_requests() {
    let capability = AsrProviderCapability::new(
        "fixture-whisper",
        "fixture-whisper@sha256:abc123",
        false,
        [AsrOutputFormat::VerboseJson, AsrOutputFormat::Srt],
        [AsrTimestampGranularity::Segment],
    )
    .unwrap();
    let digest = "a".repeat(64);
    let invalid_format = AsrRequest::new(digest.clone(), AsrOutputFormat::Srt)
        .unwrap()
        .with_timestamps([AsrTimestampGranularity::Segment]);
    assert!(capability.validate(&invalid_format).is_err());

    let unsupported_word = AsrRequest::new(digest, AsrOutputFormat::VerboseJson)
        .unwrap()
        .with_timestamps([AsrTimestampGranularity::Word]);
    assert!(capability.validate(&unsupported_word).is_err());
}
