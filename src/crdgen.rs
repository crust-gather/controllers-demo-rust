use kube::CustomResourceExt;

fn main() {
    print!(
        "{}",
        serde_yaml::to_string(&controllers_demo_rust::api::Plan::crd()).unwrap()
    )
}
