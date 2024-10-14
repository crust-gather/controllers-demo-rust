# controllers-demo-rust
Demo workshop on writing controllers in rust

We will implement a simple controller, executing commands on the host from the `Plan` custom resource, and storing command output in the status. For documentation and examples, we will follow the [docs][].

Plan example:

```yaml
apiVersion: kube.rs/v1
kind: Plan
metadata:
  name: lister
spec:
  instruction:
    retryTimes: 5
    command: ls
    args:
    - /
```

[docs]: https://kube.rs/

## Prerequisites

1. Install [rust][]
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

[rust]: https://www.rust-lang.org/tools/install

2. Initialize project

```bash
cd controllers-demo-rust
cargo init
```

3. Prepare external dependencies in `Cargo.toml`. Can follow docs on [project setup][]

<details>

```toml
[dependencies]
kube = { version = "0.96.0", features = ["runtime", "derive"] }
k8s-openapi = { version = "0.23.0", features = ["latest"] }
serde_yaml = "0.9.34"
serde = { version = "1.0.210", features = ["derive"] }
schemars = "0.8.21"
serde_json = "1.0.128"
tokio = { version = "1.40.0", features = ["full"] }
anyhow = "1.0.89"
tracing = "0.1.40"
tracing-subscriber = { version = "0.3.18", features = ["json", "env-filter"] }
thiserror = "1.0.64"
futures-util = "0.3.31"
```

</details>

[project setup]: https://kube.rs/controllers/application/#project-setup

4. Init API in `src/api.rs`.

<details>

```rust
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
#[kube(group = "kube.rs", version = "v1", kind = "Plan", plural = "plans")]
#[kube(namespaced)]
#[kube(status = "PlanStatus")]
pub struct PlanSpec {}

pub struct PlanStatus {}
```
</details>

Full API definition:

<details>

```rust
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
```

</details>

5. Add API generation in `src/crdgen.rs`. Docs on [crdgen][]

<details>

```rust
use kube::CustomResourceExt;

fn main() {
    print!(
        "{}",
        serde_yaml::to_string(&controllers_demo_rust::api::Plan::crd()).unwrap()
    )
}
```
</details>

[crdgen]: https://kube.rs/controllers/object/#installation

6. Implement `src/main.go`. Docs on [controller setup][].

<details>

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logs()?;

    let client = Client::try_default().await?;
    let plans: Api<Plan> = Api::all(client.clone());

    let context = Context { client };
    // Add controller initialization here
}

fn setup_logs() -> anyhow::Result<()> {
    let logger = tracing_subscriber::fmt::layer();
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    Registry::default()
        .with(logger)
        .with(env_filter)
        .try_init()?;

    Ok(())
}

```
</details>

[controller setup]: https://kube.rs/controllers/application/#seting-up-the-controller

7. Implement error handling in `src/main.go`: Docs on [error handling].

<details>

```rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("Command execution error: {0}")]
    Exec(#[from] std::io::Error),

    #[error("Output decode error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("Status patch error: {0}")]
    Patch(#[from] kube::Error),
}

```
</details>

[error handling]: https://kube.rs/controllers/application/#setting-up-errors

8. Implement `reconcile` method. Docs on [reconcile][].

<details>

```rust
async fn reconcile(plan: Arc<Plan>, ctx: Arc<Context>) -> Result<Action, Error> {
    info!("Reconciling plan");

    let ns = plan.namespace().unwrap_or_default();
    let api: Api<Plan> = Api::namespaced(ctx.client.clone(), &ns);
    let mut status = plan.status.clone().unwrap_or_default();

    if let Some(0) = status.result.exit_code {
        return Ok(Action::await_change());
    }

    let result = Command::new(&plan.spec.instruction.command)
        .args(plan.spec.instruction.args.clone().unwrap_or_default())
        .output()
        .await;

    if let Some(times) = plan.spec.instruction.retry_times {
        if times <= status.attempt {
            return Ok(Action::await_change());
        }
    }

    status.attempt += 1;

    match result {
        Ok(ref output) => {
            status.result.exit_code = output.status.code();
            if !output.stdout.is_empty() {
                status.result.output = Some(from_utf8(&output.stdout)?.into());
            }
            if !output.stderr.is_empty() {
                status.result.error = Some(from_utf8(&output.stderr)?.into());
            }
        }
        Err(ref err) => {
            status.result.exit_code = err.raw_os_error();
            status.result.error = Some(format!("{}", err));
        }
    };

    info!("Execution result: {:?}", status.result);

    api.patch_status(
        &plan.name_any(),
        &Default::default(),
        &Patch::Merge(serde_json::json!({"status": status})),
    )
    .await?;

    // Return error for consecutive error handling
    result?;

    Ok(Action::await_change())
}

pub fn error_policy(_plan: Arc<Plan>, err: &Error, _ctx: Arc<Context>) -> Action {
    error!("Plan execution failed: {:?}", err);

    Action::requeue(Duration::from_secs(1))
}
```

</details>

[reconcile]: https://kube.rs/controllers/application/#creating-the-reconciler