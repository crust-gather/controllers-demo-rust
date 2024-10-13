use std::{str::from_utf8, sync::Arc, time::Duration};

use controllers_demo_rust::api::Plan;
use futures_util::StreamExt as _;
use kube::{
    api::Patch,
    runtime::{controller::Action, Controller, WatchStreamExt},
    Api, Client, ResourceExt as _,
};
use thiserror::Error;
use tokio::process::Command;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Command execution error: {0}")]
    Exec(#[from] std::io::Error),

    #[error("Output decode error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("Status patch error: {0}")]
    Patch(#[from] kube::Error),
}

pub struct Context {
    pub client: Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logs()?;

    let client = Client::try_default().await?;
    let plans: Api<Plan> = Api::all(client.clone());

    let context = Context { client };

    Controller::new(plans, Default::default())
        .run(reconcile, error_policy, Arc::new(context))
        .default_backoff()
        .for_each(|_| std::future::ready(()))
        .await;

    Ok(())
}

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

pub fn error_policy(_plan: Arc<Plan>, err: &Error, _ctx: Arc<Context>) -> Action {
    error!("Plan execution failed: {:?}", err);

    Action::requeue(Duration::from_secs(1))
}
