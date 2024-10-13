use k8s_openapi::serde::{Deserialize, Serialize};
pub use kube;
use kube::CustomResource;
use schemars::JsonSchema;

/// Generate the Kubernetes wrapper struct `Plan` from our Spec and Status struct
///
/// This provides a hook for generating the CRD yaml (in crdgen.rs)
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[cfg_attr(test, derive(Default))]
#[kube(kind = "Plan", group = "kube.rs", version = "v1", namespaced)]
#[kube(status = "PlanStatus", shortname = "pl")]
pub struct PlanSpec {
    pub instruction: Instruction,
}

#[derive(Deserialize, Serialize, Default, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Instruction {
    /// Retry times for the command execution
    pub retry_times: Option<u32>,
    /// Command entrypoint.
    pub command: String,
    /// Arguments to the entrypoint.
    pub args: Option<Vec<String>>,
}

/// The status object of `Plan`
#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct PlanStatus {
    /// Execution attempt
    pub attempt: u32,
    /// Result of command execution
    pub result: InstructionOutput,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstructionOutput {
    /// Command exit code
    pub exit_code: Option<i32>,
    /// Command stdout
    pub output: Option<String>,
    /// Command stderr
    pub error: Option<String>,
}
