mod runtime;

use std::path::Path;

use async_trait::async_trait;
use lb_http_api_common::paths;
use testing_framework_core::scenario::DynError;
use testing_framework_runner_compose::{
    ComposeDeployEnv, ComposeDescriptor, DockerConfigServerSpec, NodeHostPorts, compose_runner_host,
};

use super::{LbcEnv, deployment_artifacts::add_shared_deployment_file};
use crate::{NodeHttpClient, internal::DeploymentPlan};

#[async_trait]
impl ComposeDeployEnv for LbcEnv {
    fn compose_descriptor(topology: &Self::Deployment, cfgsync_port: u16) -> ComposeDescriptor {
        let cfgsync_port = runtime::normalized_cfgsync_port(cfgsync_port);
        let (image, platform) = runtime::resolve_node_image();
        let nodes = topology
            .nodes()
            .iter()
            .enumerate()
            .map(|(index, node)| {
                runtime::build_node_descriptor(index, node, cfgsync_port, &image, platform.clone())
            })
            .collect();

        ComposeDescriptor::new(nodes)
    }

    fn cfgsync_hostnames(topology: &Self::Deployment) -> Vec<String> {
        topology_hostnames(topology)
    }

    fn enrich_cfgsync_artifacts(
        topology: &Self::Deployment,
        artifacts: &mut cfgsync_adapter::MaterializedArtifacts,
    ) -> Result<(), DynError> {
        let hostnames = topology_hostnames(topology);

        add_shared_deployment_file(topology, &hostnames, artifacts).map_err(Into::into)
    }

    fn cfgsync_container_spec(
        cfgsync_path: &Path,
        port: u16,
        network: &str,
    ) -> Result<DockerConfigServerSpec, DynError> {
        let testnet_dir = runtime::cfgsync_dir(cfgsync_path)?;
        let (image, platform) = runtime::resolve_bootstrap_image();
        let container_name = runtime::cfgsync_container_name();
        Ok(runtime::build_cfgsync_container_spec(
            &container_name,
            network,
            port,
            testnet_dir,
            &image,
            platform,
        ))
    }

    fn node_client_from_ports(
        ports: &NodeHostPorts,
        host: &str,
    ) -> Result<Self::NodeClient, DynError> {
        api_client_from_host_ports(ports, host)
    }

    fn readiness_path() -> &'static str {
        paths::CRYPTARCHIA_INFO
    }

    fn compose_runner_host() -> String {
        compose_runner_host()
    }
}

fn topology_hostnames(topology: &DeploymentPlan) -> Vec<String> {
    topology
        .nodes()
        .iter()
        .map(|node| testing_framework_runner_compose::node_identifier(node.index()))
        .collect()
}

fn api_client_from_host_ports(
    ports: &NodeHostPorts,
    host: &str,
) -> Result<NodeHttpClient, DynError> {
    let base_url = runtime::url_for_host_port(host, ports.api)?;
    let testing_url = runtime::url_for_host_port(host, ports.testing)?;
    Ok(NodeHttpClient::from_urls(base_url, Some(testing_url)))
}
