#![feature(proc_macro_hygiene, decl_macro)]

mod auth;
mod build;
mod deploy;

#[macro_use]
extern crate rocket;

use std::sync::Mutex;

use rocket::{Data, State};
use anyhow::{anyhow, Result};

use crate::auth::{ApiKey, AuthSystem, TestAuthSystem};
use crate::build::{BuildSystem, FsBuildSystem, ProjectConfig};
use crate::deploy::{DeploySystem, ServiceDeploySystem};

#[post("/deploy", data = "<crate_file>")]
fn deploy(state: State<ApiState>, crate_file: Data, api_key: ApiKey) -> Result<String> {
    // Ideally this would be done with Rocket's fairing system, but they
    // don't support state
    if !state.auth_system.authorize(&api_key)? {
        return Err(anyhow!("API key is unauthorized"));
    }

    let project = ProjectConfig {
        name: "some_project".to_string()
    };

    let build = state.build_system.build(crate_file, &api_key, &project)?;

    state.deploy_system.lock().unwrap().deploy(project.name.clone(), &build.so_path)?;

    Ok("OK".to_string())
}

struct ApiState {
    build_system: Box<dyn BuildSystem>,
    auth_system: Box<dyn AuthSystem>,
    deploy_system: Mutex<Box<dyn DeploySystem>>,
}

fn main() {
    let state = ApiState {
        build_system: Box::new(FsBuildSystem),
        auth_system: Box::new(TestAuthSystem),
        deploy_system: Mutex::new(Box::new(ServiceDeploySystem::default())),
    };

    rocket::ignite()
        .manage(state)
        .mount("/", routes![deploy]).launch();
}
