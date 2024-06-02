use std::io;

use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error("io error")]
    IoError(#[from] io::Error),
    #[error("could not create temporary file")]
    CreateTempFile(#[source] io::Error),
    #[error("post-processing failed: {0}")]
    PostProcessFailed(String),
    #[error("aws sdk error")]
    AwsS3Error(#[source] Box<aws_sdk_s3::Error>),
    #[error("s3 error")]
    S3PutObjectFailed(#[source] Box<aws_sdk_s3::Error>),
    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),
    #[error("the tool `{0}' failed healthcheck, is it installed?")]
    ToolCheckFailed(String),
}
